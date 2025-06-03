use std::time::Duration;

use crate::{
    getters::{get_ballot_box, get_guaranteed_epoch_and_slot},
    handler::CliHandler,
    instructions::{crank_post_vote, crank_vote},
    keeper::keeper_state::KeeperState,
};
use anyhow::Result;
use log::info;
use ncn_program_core::epoch_state::State;
use solana_metrics::set_host_id;
use std::process::Command;
use tokio::time::sleep;

async fn progress_epoch(
    is_epoch_completed: bool,
    current_epoch: u64,
    starting_epoch: u64,
    last_current_epoch: u64,
    keeper_epoch: u64,
) -> (u64, bool) {
    if current_epoch > last_current_epoch {
        // Automatically go to new epoch
        return (current_epoch, true);
    }

    if is_epoch_completed {
        // Reset to starting epoch
        if keeper_epoch == current_epoch {
            return (starting_epoch, false);
        }

        // Increment keeper epoch
        return (keeper_epoch + 1, false);
    }

    (keeper_epoch, false)
}

#[allow(clippy::future_not_send)]
async fn check_and_timeout_error<T>(
    title: String,
    result: &Result<T>,
    error_timeout_ms: u64,
    keeper_epoch: u64,
) -> bool {
    if let Err(e) = result {
        let error = format!("{:?}", e);
        let message = format!("Error: [{}] \n{}\n\n", title, error);

        log::error!("{}", message);
        timeout_error(error_timeout_ms).await;
        true
    } else {
        false
    }
}

async fn timeout_error(duration_ms: u64) {
    info!("Error Timeout for {}s", duration_ms as f64 / 1000.0);
    sleep(Duration::from_millis(duration_ms)).await;
    // progress_bar(duration_ms).await;
}

async fn timeout_keeper(duration_ms: u64) {
    info!("Keeper Timeout for {}s", duration_ms as f64 / 1000.0);
    sleep(Duration::from_millis(duration_ms)).await;
    // boring_progress_bar(duration_ms).await;
}

#[allow(clippy::large_stack_frames)]
#[allow(clippy::too_many_arguments)]
pub async fn startup_keeper(
    handler: &CliHandler,
    loop_timeout_ms: u64,
    error_timeout_ms: u64,
    test_vote: bool,
    metrics_only: bool,
    cluster_label: String,
    region: String,
) -> Result<()> {
    let mut state: KeeperState = KeeperState::default();
    let mut current_keeper_epoch = handler.epoch;
    let mut is_new_epoch = true;
    let (mut last_current_epoch, _) = get_guaranteed_epoch_and_slot(handler).await;

    let mut start_of_loop;
    let mut end_of_loop;

    let run_operations = !metrics_only;

    let hostname_cmd = Command::new("hostname")
        .output()
        .expect("Failed to execute hostname command");

    let hostname = String::from_utf8_lossy(&hostname_cmd.stdout)
        .trim()
        .to_string();

    set_host_id(format!(
        "ncn-operator-keeper_{}_{}_{}",
        region, cluster_label, hostname
    ));

    loop {
        // This will progress the epoch:
        // If a new Epoch turns over, it will automatically progress to it
        // If there is still work to be done on the given epoch, it will stay
        // Note: This will loop around and start back at the beginning
        {
            info!("\n\nA. Progress Epoch - {}\n", current_keeper_epoch);
            let starting_epoch = handler.epoch;
            let keeper_epoch = current_keeper_epoch;

            let (current_epoch, _) = get_guaranteed_epoch_and_slot(handler).await;
            let (result, _) = progress_epoch(
                state.is_epoch_completed,
                current_epoch,
                starting_epoch,
                last_current_epoch,
                keeper_epoch,
            )
            .await;

            if current_keeper_epoch != result {
                info!(
                    "\n\nPROGRESS EPOCH: {} -> {}\n\n",
                    current_keeper_epoch, result
                );
            }

            current_keeper_epoch = result;
            last_current_epoch = last_current_epoch.max(current_keeper_epoch);
            start_of_loop = current_keeper_epoch == handler.epoch;
            end_of_loop = current_keeper_epoch == current_epoch;
        }

        // Fetches the current state of the keeper, which holds the Epoch State
        // and other helpful information for the keeper to function
        {
            info!("\n\n0. Fetch Keeper State - {}\n", current_keeper_epoch);
            if state.epoch != current_keeper_epoch {
                let result = state.fetch(handler, current_keeper_epoch).await;

                if check_and_timeout_error(
                    "Update Keeper State".to_string(),
                    &result,
                    error_timeout_ms,
                    state.epoch,
                )
                .await
                {
                    continue;
                }
            }
        }

        // If there is no state found for the given epoch, this will create it, or
        // detect if its already been closed. Then the epoch will progress to the next
        if run_operations {
            info!("\n\n2. Check State - {}\n", current_keeper_epoch);

            // Else, if no epoch state, create it
            if state.epoch_state.is_none() {
                info!("Epoch {} does not have a state account", state.epoch);
                state.is_epoch_completed = true;
                continue;
            }
        }

        // This is where the real work is done. Depending on the state, the keeper will crank through
        // whatever is needed to be done for the given epoch.
        if run_operations {
            // Ensure epoch_state is available before calling current_state
            if state.epoch_state.is_none() {
                // This case should ideally be caught by "2. Create or Complete State"
                // If it's None here and we are in run_operations, it's an unexpected situation.
                info!("Warning: epoch_state is None in Crank State block despite run_operations being true. Epoch: {}", current_keeper_epoch);
                // Potentially continue, or rely on later checks. For now, proceed, current_state().expect might panic.
                // The "Create or Complete State" section has:
                // if state.epoch_state.is_none() { info!(...); continue; }
                // So, if run_operations is true, epoch_state should be Some.
            }

            let current_crank_state = state
                .current_state()
                .expect("epoch_state expected if run_operations");
            info!(
                "\n\n3. Crank State [{:?}] - {}\n",
                current_crank_state, current_keeper_epoch
            );

            let crank_result = match current_crank_state {
                State::SetWeight => {
                    info!(
                        "No explicit handling for {:?}. System will wait and re-evaluate.",
                        current_crank_state
                    );
                    Ok(())
                }
                State::Snapshot => {
                    info!(
                        "No explicit handling for {:?}. System will wait and re-evaluate.",
                        current_crank_state
                    );
                    Ok(())
                }
                State::Vote => {
                    let ballot_box = get_ballot_box(handler, state.epoch).await?;
                    let did_operator_vote = ballot_box.did_operator_vote(handler.operator()?)?;
                    if !did_operator_vote {
                        crank_vote(handler, state.epoch, test_vote).await?
                    } else {
                        crank_post_vote(handler, state.epoch).await?;
                        state.is_epoch_completed = true;
                    }
                    Ok(())
                }
                State::PostVoteCooldown => {
                    crank_post_vote(handler, state.epoch).await?;
                    state.is_epoch_completed = true;
                    Ok(())
                }
                State::Close => {
                    crank_post_vote(handler, state.epoch).await?;
                    state.is_epoch_completed = true;
                    Ok(())
                }
            };

            if check_and_timeout_error(
                format!("Crank State: {:?}", current_crank_state),
                &crank_result,
                error_timeout_ms,
                state.epoch,
            )
            .await
            {
                continue;
            }
        }

        // Times out the keeper - this is the main loop timeout
        if end_of_loop {
            info!("\n\nF. Timeout - {}\n", current_keeper_epoch);

            timeout_keeper(loop_timeout_ms).await;
        }
    }
}
