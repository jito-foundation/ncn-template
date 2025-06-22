use std::time::Duration;

use crate::{
    getters::get_guaranteed_epoch_and_slot,
    handler::CliHandler,
    instructions::{
        crank_close_epoch_accounts, crank_distribute, crank_post_vote_cooldown,
        crank_register_vaults, crank_set_weight, crank_snapshot, create_epoch_state,
    },
    keeper::{
        keeper_metrics::{emit_epoch_metrics, emit_error, emit_heartbeat, emit_ncn_metrics},
        keeper_state::KeeperState,
    },
};
use anyhow::Result;
use log::info;
use ncn_program_core::epoch_state::State;
use solana_metrics::set_host_id;
use std::process::Command;
use tokio::time::sleep;

/// Main entry point for the NCN (Network Coordinated Node) keeper
///
/// The keeper is responsible for progressing epoch states through their lifecycle:
/// 1. SetWeight - Set stake weights for the epoch
/// 2. Snapshot - Take snapshots of operator and vault states
/// 3. Vote - Operators vote on the epoch's outcome
/// 4. PostVoteCooldown - Wait period after voting
/// 5. Close - Close and finalize the epoch
///
/// The keeper runs in a continuous loop, handling multiple epochs and automatically
/// progressing to new epochs when the current one is complete or stalled.
///
/// # Arguments
/// * `handler` - CLI handler containing RPC client and configuration
/// * `loop_timeout_ms` - Timeout between main loop iterations when stalled
/// * `error_timeout_ms` - Timeout after errors before retrying
pub async fn startup_ncn_keeper(
    handler: &CliHandler,
    loop_timeout_ms: u64,
    error_timeout_ms: u64,
) -> Result<()> {
    let mut state: KeeperState = KeeperState::default();
    let mut epoch_stall = false;
    let mut current_keeper_epoch = handler.epoch;
    let mut tick = 0;

    let mut start_of_loop;
    let mut end_of_loop;

    // Set up metrics host identification
    let hostname_cmd = Command::new("hostname")
        .output()
        .expect("Failed to execute hostname command");

    let hostname = String::from_utf8_lossy(&hostname_cmd.stdout)
        .trim()
        .to_string();

    set_host_id(format!("ncn-program-keeper_{}", hostname));

    loop {
        // PHASE 0.1: EPOCH PROGRESSION LOGIC
        // This will progress the epoch automatically based on various conditions:
        // - If a new epoch has started on the blockchain, move to it
        // - If the current epoch has stalled, move to the next epoch
        // - If there is still work to be done on the current epoch, stay on it
        {
            info!(
                "\n\n0.1. Progress Epoch If Needed - {}\n",
                current_keeper_epoch
            );
            let starting_epoch = handler.epoch;
            let keeper_epoch = current_keeper_epoch;

            let (current_epoch, _) = get_guaranteed_epoch_and_slot(handler).await;
            let result = progress_epoch(
                state.is_epoch_completed,
                current_epoch,
                starting_epoch,
                keeper_epoch,
                epoch_stall,
            )
            .await;

            if current_keeper_epoch != result {
                info!(
                    "\n\nPROGRESS EPOCH: {} -> {}\n\n",
                    current_keeper_epoch, result
                );
            }

            current_keeper_epoch = result;
            epoch_stall = false;
            start_of_loop = current_keeper_epoch == handler.epoch;
            end_of_loop = current_keeper_epoch == current_epoch;
        }

        // PHASE 0.2: NCN METRICS EMISSION
        // Emit comprehensive metrics about the NCN state including:
        // - Validator information and status
        // - Current epoch information
        // - Ticket states and delegation information
        info!("\n\n0.2. Emit NCN Metrics - {}\n", current_keeper_epoch);
        let result = emit_ncn_metrics(handler, start_of_loop).await;

        check_and_timeout_error(
            "Emit NCN Metrics".to_string(),
            &result,
            error_timeout_ms,
            state.epoch,
        )
        .await;

        // PHASE 0.3: VAULT REGISTRATION
        // Register any outstanding vaults with the Global Vault Registry
        // This is a prerequisite for other operations and can be done at any time
        info!("\n\n0.3. Register Vaults - {}\n", current_keeper_epoch);
        let result = crank_register_vaults(handler).await;

        if check_and_timeout_error(
            "Register Vaults".to_string(),
            &result,
            error_timeout_ms,
            state.epoch,
        )
        .await
        {
            continue;
        }

        // PHASE 0.4: KEEPER STATE AND EPOCH STATE UPDATE
        // Fetch and update the keeper's internal state for the current epoch
        // This includes the EpochState account and derived information
        // We also update our local understanding of the epoch's progress
        {
            info!(
                "\n\n0.4. Fetch and Update State - {}\n",
                current_keeper_epoch
            );

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

        // PHASE 2: EPOCH STATE CREATION OR COMPLETION CHECK
        // If there's no epoch state account, create it
        // If the epoch is completed, move to the next iteration
        info!(
            "\n\n2. Create or Complete State - {}\n",
            current_keeper_epoch
        );

        // If the epoch is marked as complete, move to next iteration
        if state.is_epoch_completed {
            info!("Epoch {} is complete", state.epoch);
            continue;
        }

        // If no epoch state account exists, create it and retry
        if state.epoch_state.is_none() {
            let result = create_epoch_state(handler, state.epoch).await;

            check_and_timeout_error(
                "Create Epoch State".to_string(),
                &result,
                error_timeout_ms,
                state.epoch,
            )
            .await;

            // Continue to next iteration regardless of success/failure
            // to allow the state to be refetched
            continue;
        }

        // PHASE 3: STATE-SPECIFIC OPERATIONS
        // Execute the appropriate operations based on the current epoch state
        // Each state has specific tasks that need to be completed before progression
        let current_state = state.current_state().expect("cannot get current state");
        info!(
            "\n\n3. Crank State [{:?}] - {}\n",
            current_state, current_keeper_epoch
        );

        let result = match current_state {
            // SetWeight: Establish stake weights for all supported tokens
            State::SetWeight => crank_set_weight(handler, state.epoch).await,
            // Snapshot: Capture operator and vault state snapshots
            State::Snapshot => crank_snapshot(handler, state.epoch).await,
            // Vote: No need to do anything here
            State::Vote => {
                info!("No explicit handling for voting phase. System will wait and re-evaluate.");
                Ok(())
            }
            // PostVoteCooldown: Wait period after voting completes, this step will only log the
            // consensus result
            State::PostVoteCooldown => crank_post_vote_cooldown(handler, state.epoch).await,

            State::Distribute => crank_distribute(handler, state.epoch).await,

            // Close: Finalize and close the epoch's accounts
            State::Close => crank_close_epoch_accounts(handler, state.epoch).await,
        };

        if check_and_timeout_error(
            format!("Crank State: {:?}", current_state),
            &result,
            error_timeout_ms,
            state.epoch,
        )
        .await
        {
            continue;
        }

        // PHASE 4: EPOCH METRICS EMISSION
        // Emit detailed metrics about the current epoch's state and progress
        info!("\n\n4. Emit Epoch Metrics - {}\n", current_keeper_epoch);
        let result = emit_epoch_metrics(handler, state.epoch).await;

        check_and_timeout_error(
            "Emit Epoch Metrics".to_string(),
            &result,
            error_timeout_ms,
            state.epoch,
        )
        .await;

        // PHASE 5: STALL DETECTION
        // Detect if the epoch has stalled and should be progressed
        {
            info!("\n\n5. Detect Stall - {}\n", current_keeper_epoch);

            let result = state.detect_stall(handler).await;

            if check_and_timeout_error(
                "Detect Stall".to_string(),
                &result,
                error_timeout_ms,
                state.epoch,
            )
            .await
            {
                continue;
            }

            epoch_stall = result.unwrap();

            if epoch_stall {
                info!("\n\nSTALL DETECTED FOR {}\n\n", current_keeper_epoch);
            }
        }

        // MAIN LOOP TIMEOUT
        // If we've reached the end of processing and detected a stall,
        // wait before the next iteration and emit a heartbeat
        if end_of_loop && epoch_stall {
            info!("\n\n -- Timeout -- {}\n", current_keeper_epoch);

            timeout_keeper(loop_timeout_ms).await;
            emit_heartbeat(tick).await;
            tick += 1;
        }
    }
}

/// Determines the next epoch to process based on current conditions
///
/// This function implements the epoch progression logic:
/// - If the current epoch is completed or stalled, move to the next epoch
/// - If we've reached the blockchain's current epoch, reset to the starting epoch
/// - Otherwise, stay on the current keeper epoch
///
/// # Arguments
/// * `is_epoch_completed` - Whether the current epoch is marked as completed
/// * `current_epoch` - The current epoch according to the blockchain
/// * `starting_epoch` - The initial epoch the keeper was configured for
/// * `keeper_epoch` - The epoch the keeper is currently processing
/// * `epoch_stall` - Whether the current epoch has stalled
///
/// # Returns
/// The epoch number the keeper should process next
async fn progress_epoch(
    is_epoch_completed: bool,
    current_epoch: u64,
    starting_epoch: u64,
    keeper_epoch: u64,
    epoch_stall: bool,
) -> u64 {
    if is_epoch_completed || epoch_stall {
        // If we've caught up to the current blockchain epoch, reset to starting epoch
        if keeper_epoch == current_epoch {
            return starting_epoch;
        }

        // Otherwise, increment to the next epoch
        return keeper_epoch + 1;
    }

    // No progression needed, stay on current epoch
    keeper_epoch
}

/// Handles errors consistently across the keeper loop
///
/// This function:
/// 1. Logs errors with context
/// 2. Emits error metrics for monitoring
/// 3. Applies a timeout before allowing retry
///
/// # Arguments
/// * `title` - Description of the operation that failed
/// * `result` - The result to check for errors
/// * `error_timeout_ms` - How long to wait after an error
/// * `keeper_epoch` - Current epoch for error context
///
/// # Returns
/// `true` if an error occurred and was handled, `false` if no error
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

/// Applies a timeout after an error occurs
///
/// This prevents rapid retry attempts that could overwhelm the system
/// or hit rate limits on the RPC endpoint.
///
/// # Arguments
/// * `duration_ms` - Timeout duration in milliseconds
async fn timeout_error(duration_ms: u64) {
    info!("Error Timeout for {}s", duration_ms as f64 / 1000.0);
    sleep(Duration::from_millis(duration_ms)).await;
}

/// Applies the main keeper loop timeout
///
/// This timeout occurs when the keeper has completed all work for the current
/// epoch and is waiting for external conditions to change (e.g., new epoch,
/// operator votes, etc.).
///
/// # Arguments
/// * `duration_ms` - Timeout duration in milliseconds
async fn timeout_keeper(duration_ms: u64) {
    info!("Keeper Timeout for {}s", duration_ms as f64 / 1000.0);
    sleep(Duration::from_millis(duration_ms)).await;
}
