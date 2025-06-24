use anyhow::Result;
use ncn_program_core::ballot_box::Ballot;
use solana_metrics::datapoint_info;
use solana_sdk::pubkey::Pubkey;

use crate::{
    getters::{get_ballot_box, get_current_epoch_and_slot},
    handler::CliHandler,
};

/// Macro for emitting epoch-specific metrics
///
/// Emits metrics with two variants:
/// 1. Standard metric with the given name
/// 2. If this is the current epoch, also emits a "-current" suffixed metric
///
/// This allows tracking both historical and current epoch metrics separately
macro_rules! emit_epoch_datapoint {
    ($name:expr, $is_current_epoch:expr, $($fields:tt),*) => {
        // Always emit the standard metric
        datapoint_info!($name, $($fields),*);

        // If it's the current epoch, also emit with "-current" suffix
        if $is_current_epoch {
            datapoint_info!(
                concat!($name, "-current"),
                $($fields),*
            );
        }
    };
}

/// Emits error metrics for tracking operator failures
///
/// # Arguments
/// * `title` - The title/name of the operation that failed
/// * `error` - The error string
/// * `message` - Detailed error message
/// * `keeper_epoch` - The epoch in which the error occurred
pub async fn emit_error(title: String, error: String, message: String, keeper_epoch: u64) {
    datapoint_info!(
        "ncn-operator-keeper-error",
        ("command-title", title, String),
        ("error", error, String),
        ("message", message, String),
        ("keeper-epoch", keeper_epoch, i64),
    );
}

/// Emits heartbeat metrics to indicate the operator is alive
///
/// # Arguments
/// * `tick` - Counter representing the number of heartbeats
pub async fn emit_heartbeat(tick: u64) {
    datapoint_info!(
        "ncn-operator-keeper-keeper-heartbeat-operations",
        ("tick", tick, i64),
    );

    datapoint_info!(
        "ncn-operator-keeper-keeper-heartbeat-metrics",
        ("tick", tick, i64),
    );
}

/// Emits metrics when an operator submits a vote
///
/// # Arguments
/// * `handler` - CLI handler for RPC communication
/// * `vote` - The vote value submitted (weather status code)
/// * `epoch` - The epoch being voted on
/// * `operator` - The public key of the operator casting the vote
///
/// # Returns
/// * Result indicating success or failure
pub async fn emit_ncn_metrics_operator_vote(
    handler: &CliHandler,
    vote: u8,
    epoch: u64,
    operator: &Pubkey,
) -> Result<()> {
    let (current_epoch, current_slot) = get_current_epoch_and_slot(handler).await?;

    let is_current_epoch = current_epoch == epoch;
    emit_epoch_datapoint!(
        "ncn-operator-keeper-operator-vote",
        is_current_epoch,
        ("current-epoch", current_epoch, i64),
        ("current-slot", current_slot, i64),
        ("keeper-epoch", epoch, i64),
        ("operator", operator.to_string(), String),
        ("vote", vote as i64, i64)
    );

    Ok(())
}

/// Emits comprehensive metrics after an operator has voted
///
/// Collects and reports detailed information about:
/// - The operator's vote status
/// - Ballot box state
/// - Consensus status
/// - Vote weights
///
/// # Arguments
/// * `handler` - CLI handler for RPC communication
/// * `epoch` - The epoch being tracked
/// * `operator` - The public key of the operator
///
/// # Returns
/// * Result indicating success or failure
pub async fn emit_ncn_metrics_operator_post_vote(
    handler: &CliHandler,
    epoch: u64,
    operator: &Pubkey,
) -> Result<()> {
    let (current_epoch, current_slot) = get_current_epoch_and_slot(handler).await?;

    // Get the ballot box to determine vote status and outcomes
    let ballot_box = get_ballot_box(handler, epoch).await?;

    // Check if this operator has voted
    let did_operator_vote = ballot_box.did_operator_vote(operator);
    let operator_vote = if did_operator_vote {
        ballot_box
            .operator_votes()
            .iter()
            .find(|v| v.operator().eq(&operator))
    } else {
        None
    };

    let is_current_epoch = current_epoch == epoch;

    // Emit detailed metrics about the voting process
    emit_epoch_datapoint!(
        "ncn-operator-keeper-vote",
        is_current_epoch,
        ("current-epoch", current_epoch, i64),
        ("current-slot", current_slot, i64),
        ("keeper-epoch", epoch, i64),
        ("operator", operator.to_string(), String),
        ("has-voted", did_operator_vote as i64, i64),
        (
            "slot-voted",
            operator_vote.map_or(-1, |v| v.slot_voted() as i64),
            i64
        ),
        (
            "ballot-index",
            operator_vote.map_or(-1, |v| v.ballot_index() as i64),
            i64
        ),
        (
            "operator-weight",
            operator_vote.map_or(-1.0, |v| { v.stake_weights().stake_weight() as f64 }),
            f64
        ),
        (
            "ballot-weight",
            operator_vote.map_or(-1.0, |v| {
                let ballot = ballot_box.ballot_tallies()[v.ballot_index() as usize];
                ballot.stake_weights().stake_weight() as f64
            }),
            f64
        ),
        (
            "ballot-value",
            operator_vote.map_or(-1, |v| {
                let ballot = ballot_box.ballot_tallies()[v.ballot_index() as usize];
                ballot.ballot().weather_status() as i64
            }),
            i64
        ),
        (
            "consensus-reached",
            ballot_box.is_consensus_reached() as i64,
            i64
        ),
        (
            "winning-ballot",
            ballot_box
                .get_winning_ballot()
                .unwrap_or(&Ballot::default())
                .weather_status() as i64,
            i64
        )
    );

    Ok(())
}
