use anyhow::Result;
use ncn_program_core::ballot_box::Ballot;
use solana_metrics::datapoint_info;
use solana_sdk::{native_token::lamports_to_sol, pubkey::Pubkey};

use crate::{
    getters::{get_ballot_box, get_current_epoch_and_slot},
    handler::CliHandler,
};

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

pub const fn format_stake_weight(value: u128) -> f64 {
    value as f64
}

pub fn format_token_amount(value: u64) -> f64 {
    lamports_to_sol(value)
}

pub async fn emit_error(title: String, error: String, message: String, keeper_epoch: u64) {
    datapoint_info!(
        "ncn-operator-keeper-error",
        ("command-title", title, String),
        ("error", error, String),
        ("message", message, String),
        ("keeper-epoch", keeper_epoch, i64),
    );
}

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

pub async fn emit_ncn_metrics_operator_post_vote(
    handler: &CliHandler,
    epoch: u64,
    operator: &Pubkey,
) -> Result<()> {
    let (current_epoch, current_slot) = get_current_epoch_and_slot(handler).await?;

    let ballot_box = get_ballot_box(handler, epoch).await?;

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
