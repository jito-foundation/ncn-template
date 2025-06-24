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

/// Main operator loop that manages the NCN operator's lifecycle
///
/// This function continuously processes epochs, checking the current state
/// and performing appropriate actions based on that state (voting, post-vote actions, etc.)
///
/// # Arguments
/// * `handler` - CLI handler for RPC communication
/// * `loop_timeout_ms` - Timeout between main loop iterations in milliseconds
/// * `error_timeout_ms` - Timeout after errors in milliseconds
/// * `operator` - Public key of the operator
///
/// # Returns
/// * Result indicating success or failure (though this function loops indefinitely)
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

    // Get hostname for metrics identification
    let hostname_cmd = Command::new("hostname")
        .output()
        .expect("Failed to execute hostname command");

    let hostname = String::from_utf8_lossy(&hostname_cmd.stdout)
        .trim()
        .to_string();

    // Set host ID for metrics collection
    set_host_id(format!("ncn-operator-keeper_{}", hostname));

    loop {
        // Progress to next epoch if needed
        // If a new epoch has started, advance to it
        // If there's still work in the current epoch, stay on it
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

        // Keeper state and epoch state update
        // Fetch and update the keeper's internal state for the current epoch
        // This includes the EpochState account and derived information
        // We also update our local understanding of the epoch's progress
        {
            info!("\n\n0. Fetch and Update State - {}\n", current_keeper_epoch);

            // If the epoch has changed, fetch the new epoch state
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
            } else {
                // Otherwise, just update the existing epoch state
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
            }
        }

        // Check the current state and perform appropriate actions
        {
            info!("\n\n2. Check State - {}\n", current_keeper_epoch);

            // If no epoch state exists, mark as completed and continue
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

            // Handle different epoch states with appropriate actions
            let crank_result = match current_crank_state {
                // Weight and Snapshot states are passive - no operator action needed
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
                // Vote state - operator casts a vote if eligible
                State::Vote => {
                    // Get the ballot box and operator snapshot for the current epoch
                    let ballot_box = get_ballot_box(handler, state.epoch).await?;
                    let operator_snapshot =
                        get_operator_snapshot(handler, &operator, state.epoch).await?;

                    // Check if this operator is eligible to vote in this epoch
                    let can_operator_vote =
                        can_operator_vote(ballot_box, operator_snapshot, &operator);

                    if can_operator_vote {
                        // If operator can vote:
                        // 1. Cast the vote
                        let result = operator_crank_vote(handler, state.epoch, &operator).await;

                        // 2. Handle any errors that occurred during voting
                        check_and_timeout_error(
                            "Operator Casting a Vote".to_string(),
                            &result,
                            error_timeout_ms,
                            state.epoch,
                        )
                        .await;

                        // 3. Emit metrics about the vote
                        info!(
                            "\n\n Emit Epoch NCN Operator Vote Metrics - {}\n",
                            current_keeper_epoch
                        );
                        // Use vote result or default to 3 if vote failed
                        let vote = result.unwrap_or(3);
                        let result =
                            emit_ncn_metrics_operator_vote(handler, vote, state.epoch, &operator)
                                .await;

                        // 4. Handle any errors from metrics emission
                        check_and_timeout_error(
                            "Emit NCN Operator Vote metrics".to_string(),
                            &result,
                            error_timeout_ms,
                            state.epoch,
                        )
                        .await;
                    } else {
                        // If operator cannot vote (already voted or not eligible):
                        // 1. Perform post-vote actions
                        operator_crank_post_vote(handler, state.epoch, &operator).await?;

                        // 2. Emit metrics about the post-vote state
                        info!(
                            "\n\n Emit Epoch post vote metrics - {}\n",
                            current_keeper_epoch
                        );
                        let result =
                            emit_ncn_metrics_operator_post_vote(handler, state.epoch, &operator)
                                .await;

                        // 3. Handle any errors from metrics emission
                        check_and_timeout_error(
                            "Emit NCN Operator Post Vote Metrics".to_string(),
                            &result,
                            error_timeout_ms,
                            state.epoch,
                        )
                        .await;

                        // 4. Mark this epoch as completed for this operator
                        state.is_epoch_completed = true;
                    }
                    Ok(())
                }
                // Post-vote states - perform post-vote actions and mark epoch as completed
                State::PostVoteCooldown | State::Close => {
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

        // Main loop timing control - add delay between iterations
        if end_of_loop {
            info!("\n\nF. Timeout - {}\n", current_keeper_epoch);

            timeout_keeper(loop_timeout_ms).await;

            // Emit heartbeat metric to indicate the operator is alive
            emit_heartbeat(tick).await;
            tick += 1;
        }
    }
}

/// Determines whether to progress to the next epoch
///
/// Logic for advancing the keeper's current epoch:
/// - If current epoch is completed and matches the blockchain epoch, reset to starting epoch
/// - If current epoch is completed, increment to next epoch
/// - If current epoch is not completed, stay on it
///
/// # Arguments
/// * `is_epoch_completed` - Whether the current epoch is completed
/// * `current_epoch` - The current blockchain epoch
/// * `starting_epoch` - The epoch the keeper started with
/// * `keeper_epoch` - The keeper's current epoch
///
/// # Returns
/// * The new epoch number for the keeper
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

/// Checks for errors and implements timeouts if errors occur
///
/// # Arguments
/// * `title` - Description of the operation being checked
/// * `result` - The result to check for errors
/// * `error_timeout_ms` - Milliseconds to wait if an error is found
/// * `keeper_epoch` - Current epoch for error reporting
///
/// # Returns
/// * Boolean indicating whether an error was found (true = error)
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

/// Implements a timeout delay after an error occurs
///
/// # Arguments
/// * `duration_ms` - Milliseconds to wait
async fn timeout_error(duration_ms: u64) {
    info!("Error Timeout for {}s", duration_ms as f64 / 1000.0);
    sleep(Duration::from_millis(duration_ms)).await;
    // progress_bar(duration_ms).await; // Commented out progress bar
}

/// Implements a timeout delay between keeper loop iterations
///
/// # Arguments
/// * `duration_ms` - Milliseconds to wait
async fn timeout_keeper(duration_ms: u64) {
    info!("Keeper Timeout for {}s", duration_ms as f64 / 1000.0);
    sleep(Duration::from_millis(duration_ms)).await;
    // boring_progress_bar(duration_ms).await; // Commented out progress bar
}
