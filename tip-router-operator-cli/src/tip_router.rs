use anyhow::Result;
use ellipsis_client::{ClientSubset, EllipsisClient, EllipsisClientResult};
use jito_bytemuck::AccountDeserialize;
use jito_tip_router_client::instructions::CastVoteBuilder;
use jito_tip_router_core::{
    ballot_box::BallotBox,
    config::Config,
    epoch_snapshot::{EpochSnapshot, OperatorSnapshot},
    epoch_state::EpochState,
};
use log::info;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signature},
    signer::Signer,
    transaction::Transaction,
};

/// Fetch and deserialize
pub async fn get_ncn_config(
    client: &EllipsisClient,
    tip_router_program_id: &Pubkey,
    ncn_pubkey: &Pubkey,
) -> Result<Config> {
    let config_pda = Config::find_program_address(tip_router_program_id, ncn_pubkey).0;
    let config = client.get_account(&config_pda).await?;
    Ok(*Config::try_from_slice_unchecked(config.data.as_slice()).unwrap())
}

/// Generate and send a CastVote instruction with the merkle root.
#[allow(clippy::too_many_arguments)]
pub async fn cast_vote(
    client: &EllipsisClient,
    payer: &Keypair,
    tip_router_program_id: &Pubkey,
    ncn: &Pubkey,
    operator: &Pubkey,
    operator_voter: &Keypair,
    weather_status: u8,
    tip_router_epoch: u64,
    submit_as_memo: bool,
) -> EllipsisClientResult<Signature> {
    let epoch_state =
        EpochState::find_program_address(tip_router_program_id, ncn, tip_router_epoch).0;

    let ncn_config = Config::find_program_address(tip_router_program_id, ncn).0;

    let ballot_box =
        BallotBox::find_program_address(tip_router_program_id, ncn, tip_router_epoch).0;

    let epoch_snapshot =
        EpochSnapshot::find_program_address(tip_router_program_id, ncn, tip_router_epoch).0;

    let operator_snapshot = OperatorSnapshot::find_program_address(
        tip_router_program_id,
        operator,
        ncn,
        tip_router_epoch,
    )
    .0;

    let ix = if submit_as_memo {
        spl_memo::build_memo(&[weather_status], &[&operator_voter.pubkey()])
    } else {
        CastVoteBuilder::new()
            .epoch_state(epoch_state)
            .config(ncn_config)
            .ballot_box(ballot_box)
            .ncn(*ncn)
            .epoch_snapshot(epoch_snapshot)
            .operator_snapshot(operator_snapshot)
            .operator(*operator)
            .operator_voter(operator_voter.pubkey())
            .weather_status(weather_status)
            .epoch(tip_router_epoch)
            .instruction()
    };

    info!("Submitting meta merkle root {:?}", weather_status);

    let tx = Transaction::new_with_payer(&[ix], Some(&payer.pubkey()));
    client
        .process_transaction(tx, &[payer, operator_voter])
        .await
}
