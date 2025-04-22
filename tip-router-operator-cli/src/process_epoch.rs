use std::{path::PathBuf, str::FromStr, time::Duration};

use anyhow::Result;
use ellipsis_client::EllipsisClient;
use log::{error, info};
use solana_metrics::datapoint_info;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{epoch_info::EpochInfo, pubkey::Pubkey, signature::read_keypair_file};
use tokio::time;

use crate::{backup_snapshots::SnapshotInfo, submit::submit_to_ncn, Cli, OperatorState, Version};

const MAX_WAIT_FOR_INCREMENTAL_SNAPSHOT_TICKS: u64 = 1200; // Experimentally determined
const OPTIMAL_INCREMENTAL_SNAPSHOT_SLOT_RANGE: u64 = 800; // Experimentally determined

pub async fn wait_for_next_epoch(rpc_client: &RpcClient, current_epoch: u64) -> EpochInfo {
    loop {
        tokio::time::sleep(Duration::from_secs(10)).await; // Check every 10 seconds
        let new_epoch_info = match rpc_client.get_epoch_info().await {
            Ok(info) => info,
            Err(e) => {
                error!("Error getting epoch info: {:?}", e);
                continue;
            }
        };

        if new_epoch_info.epoch > current_epoch {
            info!(
                "New epoch detected: {} -> {}",
                current_epoch, new_epoch_info.epoch
            );
            return new_epoch_info;
        }
    }
}

pub async fn get_previous_epoch_last_slot(rpc_client: &RpcClient) -> Result<(u64, u64)> {
    let epoch_info = rpc_client.get_epoch_info().await?;
    calc_prev_epoch_and_final_slot(&epoch_info)
}

pub fn calc_prev_epoch_and_final_slot(epoch_info: &EpochInfo) -> Result<(u64, u64)> {
    let current_slot = epoch_info.absolute_slot;
    let slot_index = epoch_info.slot_index;

    // Handle case where we're in the first epoch
    if current_slot < slot_index {
        return Ok((0, 0));
    }

    let epoch_start_slot = current_slot
        .checked_sub(slot_index)
        .ok_or_else(|| anyhow::anyhow!("epoch_start_slot subtraction overflow"))?;
    let previous_epoch_final_slot = epoch_start_slot.saturating_sub(1);
    let previous_epoch = epoch_info.epoch.saturating_sub(1);

    Ok((previous_epoch, previous_epoch_final_slot))
}

/// Wait for the optimal incremental snapshot to be available to speed up full snapshot generation
/// Automatically returns after MAX_WAIT_FOR_INCREMENTAL_SNAPSHOT_TICKS seconds
pub async fn wait_for_optimal_incremental_snapshot(
    incremental_snapshots_dir: PathBuf,
    target_slot: u64,
) -> Result<()> {
    let mut interval = time::interval(Duration::from_secs(1));
    let mut ticks = 0;

    while ticks < MAX_WAIT_FOR_INCREMENTAL_SNAPSHOT_TICKS {
        let dir_entries = std::fs::read_dir(&incremental_snapshots_dir)?;

        for entry in dir_entries {
            if let Some(snapshot_info) = SnapshotInfo::from_path(entry?.path()) {
                if target_slot - OPTIMAL_INCREMENTAL_SNAPSHOT_SLOT_RANGE < snapshot_info.end_slot
                    && snapshot_info.end_slot <= target_slot
                {
                    return Ok(());
                }
            }
        }

        interval.tick().await;
        ticks += 1;
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn loop_stages(
    rpc_client: EllipsisClient,
    cli: Cli,
    starting_stage: OperatorState,
    override_target_slot: Option<u64>,
    tip_router_program_id: &Pubkey,
    ncn_address: &Pubkey,
    _enable_snapshots: bool,
) -> Result<()> {
    let keypair = read_keypair_file(&cli.keypair_path).expect("Failed to read keypair file");
    let mut current_epoch_info = rpc_client.get_epoch_info().await?;

    // Track runs that are starting right at the beginning of a new epoch
    let operator_address = cli.operator_address.clone();
    let mut stage = starting_stage;
    let mut epoch_to_process = current_epoch_info.epoch.saturating_sub(1);
    let mut slot_to_process = if let Some(slot) = override_target_slot {
        slot
    } else {
        let (_, prev_slot) = calc_prev_epoch_and_final_slot(&current_epoch_info)?;
        prev_slot
    };
    loop {
        match stage {
            OperatorState::LoadBankFromSnapshot => {
                let incremental_snapshots_path = cli.backup_snapshots_dir.clone();
                wait_for_optimal_incremental_snapshot(incremental_snapshots_path, slot_to_process)
                    .await?;

                // Transition to the next stage
                stage = OperatorState::CreateMetaMerkleTree;
            }
            OperatorState::CreateMetaMerkleTree => {
                datapoint_info!(
                    "tip_router_cli.process_epoch",
                    ("operator_address", operator_address, String),
                    ("epoch", epoch_to_process, i64),
                    ("status", "success", String),
                    ("state", "epoch_processing_completed", String),
                    ("version", Version::default().to_string(), String),
                );
                stage = OperatorState::CastVote;
            }
            OperatorState::CastVote => {
                let operator_address = Pubkey::from_str(&cli.operator_address)?;
                submit_to_ncn(
                    &rpc_client,
                    &keypair,
                    &operator_address,
                    epoch_to_process,
                    ncn_address,
                    tip_router_program_id,
                    [1; 32],
                    cli.submit_as_memo,
                )
                .await?;
                stage = OperatorState::WaitForNextEpoch;
            }
            OperatorState::WaitForNextEpoch => {
                current_epoch_info =
                    wait_for_next_epoch(&rpc_client, current_epoch_info.epoch).await;
                // Get the last slot of the previous epoch
                let (previous_epoch, previous_epoch_slot) =
                    if let Ok((epoch, slot)) = get_previous_epoch_last_slot(&rpc_client).await {
                        (epoch, slot)
                    } else {
                        // TODO: Make a datapoint error
                        error!("Error getting previous epoch slot");
                        continue;
                    };
                slot_to_process = previous_epoch_slot;
                epoch_to_process = previous_epoch;

                stage = OperatorState::LoadBankFromSnapshot;
            }
        }
    }
}
