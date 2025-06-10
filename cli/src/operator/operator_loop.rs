use std::time::Duration;

use crate::{
    getters::{get_ballot_box, get_guaranteed_epoch_and_slot, get_operator_snapshot},
    handler::CliHandler,
    instructions::{operator_crank_post_vote, operator_crank_vote},
    operator::{
        operator_metrics::{
            emit_error, emit_heartbeat, emit_ncn_metrics_operator_post_vote,
            emit_ncn_metrics_operator_vote,
        },
        operator_state::KeeperState,
    },
};
use anyhow::Result;
use log::info;
use ncn_program_core::{epoch_state::State, utils::can_operator_vote};
use solana_metrics::set_host_id;
use solana_sdk::pubkey::Pubkey;
use std::process::Command;
use tokio::time::sleep;

#[allow(clippy::large_stack_frames)]
pub async fn startup_operator_loop(
    handler: &CliHandler,
    loop_timeout_ms: u64,
    error_timeout_ms: u64,
    operator: Pubkey,
) -> Result<()> {
    let mut state: KeeperState = KeeperState::default();
    let mut current_keeper_epoch = handler.epoch;
    let mut tick = 0;

    let mut end_of_loop;

    let hostname_cmd = Command::new("hostname")
        .output()
        .expect("Failed to execute hostname command");

    let hostname = String::from_utf8_lossy(&hostname_cmd.stdout)
        .trim()
        .to_string();

    set_host_id(format!("ncn-operator-keeper_{}", hostname));

    loop {
        // This will progress the epoch:
        // If a new Epoch turns over, it will automatically progress to it
        // If there is still work to be done on the given epoch, it will stay
        // Note: This will loop around and start back at the beginning
        {
            info!(
                "\n\n0. Progress Epoch If Needed - {}\n",
                current_keeper_epoch
            );
            let starting_epoch = handler.epoch;

            let (current_epoch, _) = get_guaranteed_epoch_and_slot(handler).await;
            let result = progress_epoch(
                state.is_epoch_completed,
                current_epoch,
                starting_epoch,
                current_keeper_epoch,
            )
            .await;

            if current_keeper_epoch != result {
                info!(
                    "\n\nPROGRESS EPOCH: {} -> {}\n\n",
                    current_keeper_epoch, result
                );
            }

            current_keeper_epoch = result;
            end_of_loop = current_keeper_epoch == current_epoch;
        }

        // Fetches the current state of the keeper, which holds the Epoch State
        // and other helpful information for the keeper to function
        {
            info!("\n\n0. Fetch Keeper State - {}\n", current_keeper_epoch);
            if state.epoch != current_keeper_epoch {
                let result = state.fetch(handler, current_keeper_epoch).await;

                if check_and_timeout_error(
                    "Fetch Keeper State".to_string(),
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

        // Updates the Epoch State - pulls from the Epoch State account from on chain
        // and further updates the keeper state
        {
            info!("\n\n1. Update Epoch State - {}\n", current_keeper_epoch);
            let result = state.update_epoch_state(handler).await;

            if check_and_timeout_error(
                "Update Epoch State".to_string(),
                &result,
                error_timeout_ms,
                state.epoch,
            )
            .await
            {
                continue;
            }

            // If complete, reset loop
            if state.is_epoch_completed {
                info!("Epoch {} is complete", state.epoch);
                continue;
            }
        }

        {
            info!("\n\n2. Check State - {}\n", current_keeper_epoch);

            if state.epoch_state.is_none() {
                info!("Epoch {} does not have a state account", state.epoch);
                state.is_epoch_completed = true;
                continue;
            }

            let current_crank_state = state.current_state().expect("epoch_state expected");

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
                    let operator_snapshot =
                        get_operator_snapshot(handler, &operator, state.epoch).await?;
                    let can_operator_vote =
                        can_operator_vote(ballot_box, operator_snapshot, &operator);
                    if can_operator_vote {
                        let result = operator_crank_vote(handler, state.epoch, &operator).await;
                        check_and_timeout_error(
                            "Operator Casting a Vote".to_string(),
                            &result,
                            error_timeout_ms,
                            state.epoch,
                        )
                        .await;

                        info!(
                            "\n\n Emit Epoch NCN Operator Vote Metrics - {}\n",
                            current_keeper_epoch
                        );
                        let vote = result.unwrap_or(3);
                        let result =
                            emit_ncn_metrics_operator_vote(handler, vote, state.epoch, &operator)
                                .await;

                        check_and_timeout_error(
                            "Emit NCN Operator Vote metrics".to_string(),
                            &result,
                            error_timeout_ms,
                            state.epoch,
                        )
                        .await;
                    } else {
                        operator_crank_post_vote(handler, state.epoch, &operator).await?;

                        info!(
                            "\n\n Emit Epoch post vote metrics - {}\n",
                            current_keeper_epoch
                        );
                        let result =
                            emit_ncn_metrics_operator_post_vote(handler, state.epoch, &operator)
                                .await;

                        check_and_timeout_error(
                            "Emit NCN Operator Post Vote Metrics".to_string(),
                            &result,
                            error_timeout_ms,
                            state.epoch,
                        )
                        .await;
                        state.is_epoch_completed = true;
                    }
                    Ok(())
                }
                State::PostVoteCooldown => {
                    operator_crank_post_vote(handler, state.epoch, &operator).await?;

                    info!(
                        "\n\n Emit Epoch post vote metrics - {}\n",
                        current_keeper_epoch
                    );
                    let result =
                        emit_ncn_metrics_operator_post_vote(handler, state.epoch, &operator).await;

                    check_and_timeout_error(
                        "Emit NCN Operator Post Vote Metrics".to_string(),
                        &result,
                        error_timeout_ms,
                        state.epoch,
                    )
                    .await;
                    state.is_epoch_completed = true;
                    Ok(())
                }
                State::Close => {
                    operator_crank_post_vote(handler, state.epoch, &operator).await?;

                    info!(
                        "\n\n Emit Epoch post vote metrics - {}\n",
                        current_keeper_epoch
                    );
                    let result =
                        emit_ncn_metrics_operator_post_vote(handler, state.epoch, &operator).await;

                    check_and_timeout_error(
                        "Emit NCN Operator Post Vote Metrics".to_string(),
                        &result,
                        error_timeout_ms,
                        state.epoch,
                    )
                    .await;
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

            emit_heartbeat(tick).await;
            tick += 1;
        }
    }
}

async fn progress_epoch(
    is_epoch_completed: bool,
    current_epoch: u64,
    starting_epoch: u64,
    keeper_epoch: u64,
) -> u64 {
    if is_epoch_completed {
        // Reset to starting epoch
        if keeper_epoch == current_epoch {
            return starting_epoch;
        }

        // Increment keeper epoch
        return keeper_epoch + 1;
    }

    keeper_epoch
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
        emit_error(title, error, message, keeper_epoch).await;
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
