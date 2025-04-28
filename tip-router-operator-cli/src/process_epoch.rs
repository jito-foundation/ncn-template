use std::{path::PathBuf, str::FromStr, time::Duration};

use anyhow::Result;
use ellipsis_client::EllipsisClient;
use jito_tip_router_core::ballot_box::WeatherStatus;
use log::{error, info};
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{epoch_info::EpochInfo, pubkey::Pubkey, signature::read_keypair_file};

use crate::{submit::submit_to_ncn, Cli, OperatorState};

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

pub async fn get_previous_epoch_last_slot(rpc_client: &RpcClient) -> Result<u64> {
    let epoch_info = rpc_client.get_epoch_info().await?;
    calc_prev_epoch_and_final_slot(&epoch_info)
}

pub fn calc_prev_epoch_and_final_slot(epoch_info: &EpochInfo) -> Result<u64> {
    let current_slot = epoch_info.absolute_slot;
    let slot_index = epoch_info.slot_index;

    // Handle case where we're in the first epoch
    if current_slot < slot_index {
        return Ok(0);
    }

    let previous_epoch = epoch_info.epoch.saturating_sub(1);

    Ok(previous_epoch)
}

#[allow(clippy::too_many_arguments)]
pub async fn loop_stages(
    rpc_client: EllipsisClient,
    cli: Cli,
    starting_stage: OperatorState,
    tip_router_program_id: &Pubkey,
    ncn_address: &Pubkey,
    _enable_snapshots: bool,
) -> Result<()> {
    let keypair = read_keypair_file(&cli.keypair_path).expect("Failed to read keypair file");
    let mut current_epoch_info = rpc_client.get_epoch_info().await?;

    let mut stage = starting_stage;
    let mut epoch_to_process = current_epoch_info.epoch.saturating_sub(1);
    loop {
        match stage {
            OperatorState::CastVote => {
                let operator_address = Pubkey::from_str(&cli.operator_address)?;
                submit_to_ncn(
                    &rpc_client,
                    &keypair,
                    &operator_address,
                    epoch_to_process,
                    ncn_address,
                    tip_router_program_id,
                    WeatherStatus::default() as u8,
                    cli.submit_as_memo,
                )
                .await?;
                stage = OperatorState::WaitForNextEpoch;
            }
            OperatorState::WaitForNextEpoch => {
                current_epoch_info =
                    wait_for_next_epoch(&rpc_client, current_epoch_info.epoch).await;
                // Get the last slot of the previous epoch
                let previous_epoch =
                    if let Ok(epoch) = get_previous_epoch_last_slot(&rpc_client).await {
                        epoch
                    } else {
                        // TODO: Make a datapoint error
                        error!("Error getting previous epoch slot");
                        continue;
                    };
                epoch_to_process = previous_epoch;

                stage = OperatorState::CastVote;
            }
        }
    }
}
