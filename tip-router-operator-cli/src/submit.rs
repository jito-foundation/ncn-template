use std::sync::Arc;
use std::{path::PathBuf, str::FromStr};

use ellipsis_client::EllipsisClient;
use jito_bytemuck::AccountDeserialize as JitoAccountDeserialize;
use jito_tip_router_core::{ballot_box::BallotBox, config::Config};
use log::{debug, error, info};
use meta_merkle_tree::meta_merkle_tree::MetaMerkleTree;
use solana_metrics::{datapoint_error, datapoint_info};
use solana_sdk::{pubkey::Pubkey, signature::Keypair};

use crate::{meta_merkle_tree_file_name, Version};
use crate::{
    tip_router::{cast_vote, get_ncn_config},
    Cli,
};

#[allow(clippy::too_many_arguments)]
pub async fn submit_recent_epochs_to_ncn(
    client: &EllipsisClient,
    keypair: &Arc<Keypair>,
    ncn_address: &Pubkey,
    tip_router_program_id: &Pubkey,
    tip_distribution_program_id: &Pubkey,
    num_monitored_epochs: u64,
    cli_args: &Cli,
    set_merkle_roots: bool,
) -> Result<(), anyhow::Error> {
    let epoch = client.get_epoch_info().await?;
    let operator_address = Pubkey::from_str(&cli_args.operator_address)?;

    for i in 0..num_monitored_epochs {
        let process_epoch = epoch.epoch.checked_sub(i).unwrap();

        let meta_merkle_tree_dir = cli_args.get_save_path();
        let target_meta_merkle_tree_file = meta_merkle_tree_file_name(process_epoch);
        let target_meta_merkle_tree_path = meta_merkle_tree_dir.join(target_meta_merkle_tree_file);
        if !target_meta_merkle_tree_path.exists() {
            continue;
        }

        match submit_to_ncn(
            client,
            keypair,
            &operator_address,
            &target_meta_merkle_tree_path,
            process_epoch,
            ncn_address,
            tip_router_program_id,
            tip_distribution_program_id,
            cli_args.submit_as_memo,
            set_merkle_roots,
        )
        .await
        {
            Ok(_) => {}
            Err(e) => error!("Failed to submit epoch {} to NCN: {:?}", process_epoch, e),
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn submit_to_ncn(
    client: &EllipsisClient,
    keypair: &Keypair,
    operator_address: &Pubkey,
    meta_merkle_tree_path: &PathBuf,
    merkle_root_epoch: u64,
    ncn_address: &Pubkey,
    tip_router_program_id: &Pubkey,
    tip_distribution_program_id: &Pubkey,
    submit_as_memo: bool,
    set_merkle_roots: bool,
) -> Result<(), anyhow::Error> {
    let epoch_info = client.get_epoch_info().await?;
    let meta_merkle_tree = MetaMerkleTree::new_from_file(meta_merkle_tree_path)?;
    let config = get_ncn_config(client, tip_router_program_id, ncn_address).await?;

    // The meta merkle root files are tagged with the epoch they have created the snapshot for
    // Tip router accounts for that merkle root are created in the next epoch
    let tip_router_target_epoch = merkle_root_epoch + 1;

    // Check for ballot box
    let ballot_box_address = BallotBox::find_program_address(
        tip_router_program_id,
        ncn_address,
        tip_router_target_epoch,
    )
    .0;

    let ballot_box_account = match client.get_account(&ballot_box_address).await {
        Ok(account) => account,
        Err(e) => {
            debug!(
                "Ballot box not created yet for epoch {}: {:?}",
                tip_router_target_epoch, e
            );
            return Ok(());
        }
    };

    let ballot_box = BallotBox::try_from_slice_unchecked(&ballot_box_account.data)?;

    let is_voting_valid = ballot_box.is_voting_valid(
        epoch_info.absolute_slot,
        config.valid_slots_after_consensus(),
    )?;

    // If exists, look for vote from current operator
    let vote = ballot_box
        .operator_votes()
        .iter()
        .find(|vote| vote.operator() == operator_address);

    let should_cast_vote = match vote {
        Some(vote) => {
            // If vote exists, cast_vote if different from current meta_merkle_root
            let tally = ballot_box
                .ballot_tallies()
                .get(vote.ballot_index() as usize)
                .ok_or_else(|| anyhow::anyhow!("Ballot tally not found"))?;

            tally.ballot().root() != meta_merkle_tree.merkle_root
        }
        None => true,
    };

    info!(
        "Determining if operator needs to vote...\n\
        should_cast_vote: {}\n\
        is_voting_valid: {}
        ",
        should_cast_vote, is_voting_valid
    );

    if should_cast_vote && is_voting_valid {
        let res = cast_vote(
            client,
            keypair,
            tip_router_program_id,
            ncn_address,
            operator_address,
            keypair,
            meta_merkle_tree.merkle_root,
            tip_router_target_epoch,
            submit_as_memo,
        )
        .await;

        match res {
            Ok(signature) => {
                datapoint_info!(
                    "tip_router_cli.vote_cast",
                    ("operator_address", operator_address.to_string(), String),
                    ("epoch", tip_router_target_epoch, i64),
                    (
                        "merkle_root",
                        format!("{:?}", meta_merkle_tree.merkle_root),
                        String
                    ),
                    ("version", Version::default().to_string(), String),
                    ("tx_sig", format!("{:?}", signature), String)
                );
                info!(
                    "Cast vote for epoch {} with signature {:?}",
                    tip_router_target_epoch, signature
                )
            }
            Err(e) => {
                datapoint_error!(
                    "tip_router_cli.vote_cast",
                    ("operator_address", operator_address.to_string(), String),
                    ("epoch", tip_router_target_epoch, i64),
                    (
                        "merkle_root",
                        format!("{:?}", meta_merkle_tree.merkle_root),
                        String
                    ),
                    ("status", "error", String),
                    ("error", format!("{:?}", e), String)
                );
                info!(
                    "Failed to cast vote for epoch {}: {:?}",
                    tip_router_target_epoch, e
                )
            }
        }
    }

    Ok(())
}
