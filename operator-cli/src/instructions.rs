use std::time::Duration;

use crate::{
    getters::{get_account, get_ballot_box, get_consensus_result},
    handler::CliHandler,
    log::boring_progress_bar,
};
use anyhow::{anyhow, Ok, Result};

use log::info;

use ncn_program_client::instructions::CastVoteBuilder;
use ncn_program_core::{
    ballot_box::{BallotBox, WeatherStatus},
    config::Config as NCNProgramConfig,
    consensus_result::ConsensusResult,
    epoch_snapshot::{EpochSnapshot, OperatorSnapshot},
    epoch_state::EpochState,
};
use solana_client::rpc_config::RpcSendTransactionConfig;

use solana_sdk::{
    compute_budget::ComputeBudgetInstruction,
    instruction::Instruction,
    msg,
    pubkey::Pubkey,
    signature::{Keypair, Signature},
    signer::Signer,
    transaction::Transaction,
};
use tokio::time::sleep;

// --------------------- operator ------------------------------

pub async fn operator_cast_vote(
    handler: &CliHandler,
    epoch: u64,
    weather_status: u8,
) -> Result<()> {
    let keypair = handler.keypair()?;

    let ncn = *handler.ncn()?;

    let operator = *handler.operator()?;

    let (config, _, _) = NCNProgramConfig::find_program_address(&handler.ncn_program_id, &ncn);

    let (epoch_state, _, _) =
        EpochState::find_program_address(&handler.ncn_program_id, &ncn, epoch);

    let (ballot_box, _, _) = BallotBox::find_program_address(&handler.ncn_program_id, &ncn, epoch);

    let (epoch_snapshot, _, _) =
        EpochSnapshot::find_program_address(&handler.ncn_program_id, &ncn, epoch);

    let (operator_snapshot, _, _) =
        OperatorSnapshot::find_program_address(&handler.ncn_program_id, &operator, &ncn, epoch);
    let (consensus_result, _, _) =
        ConsensusResult::find_program_address(&handler.ncn_program_id, &ncn, epoch);

    let cast_vote_ix = CastVoteBuilder::new()
        .config(config)
        .epoch_state(epoch_state)
        .ballot_box(ballot_box)
        .ncn(ncn)
        .epoch_snapshot(epoch_snapshot)
        .operator_snapshot(operator_snapshot)
        .operator(operator)
        .operator_voter(keypair.pubkey())
        .consensus_result(consensus_result)
        .weather_status(weather_status)
        .epoch(epoch)
        .instruction();

    send_and_log_transaction(
        handler,
        &[cast_vote_ix],
        &[],
        "Cast Vote",
        &[
            format!("NCN: {:?}", ncn),
            format!("Operator: {:?}", operator),
            format!(
                "Meta Merkle Root: {:?}",
                WeatherStatus::from_u8(weather_status)
            ),
            format!("Epoch: {:?}", epoch),
        ],
    )
    .await?;

    Ok(())
}

pub async fn crank_vote(handler: &CliHandler, epoch: u64, test_vote: bool) -> Result<()> {
    // TODO: implement voting logic

    let ballot_box = get_ballot_box(handler, epoch).await?;
    msg!("Ballot Box for epoch {}: {}", epoch, ballot_box);
    let did_operator_vote = ballot_box.did_operator_vote(handler.operator()?)?;
    msg!("did_operator_vote {}", did_operator_vote);
    msg!("operator votes {:?}", ballot_box.operators_voted());

    if ballot_box.is_consensus_reached() {
        log::info!(
            "Consensus already reached for epoch: {:?}. Skipping voting.",
            epoch
        );
        return Ok(());
    }

    if test_vote {
        operator_cast_vote(handler, epoch, 1).await?;
    }

    Ok(())
}

pub async fn crank_post_vote(handler: &CliHandler, epoch: u64) -> Result<()> {
    let ballot_box = get_ballot_box(handler, epoch).await?;
    let operator = *handler.operator()?;

    let did_operator_vote = ballot_box.did_operator_vote(handler.operator()?)?;
    let operator_vote = if did_operator_vote {
        ballot_box
            .operator_votes()
            .iter()
            .find(|v| v.operator().eq(&operator))
    } else {
        None
    };

    let mut log_message = format!("\n----- Post Vote Status -----\n");
    log_message.push_str(&format!("Epoch: {}\n", epoch));
    log_message.push_str(&format!("Did Operator Vote: {}\n", did_operator_vote));

    if let Some(vote) = operator_vote {
        let operator_ballot = ballot_box.ballot_tallies()[vote.ballot_index() as usize];
        let operator_ballot_weight = operator_ballot.stake_weights();
        log_message.push_str("Operator Vote Details:\n");
        log_message.push_str(&format!("  Operator: {}\n", vote.operator()));
        log_message.push_str(&format!("  Slot Voted: {}\n", vote.slot_voted()));
        log_message.push_str(&format!("  Ballot Index: {}\n", vote.ballot_index()));
        log_message.push_str(&format!(
            "  Operator Ballot Weight: {}\n",
            operator_ballot_weight.stake_weight()
        ));
        log_message.push_str(&format!("  Operator Vote: {}\n", operator_ballot.ballot()));
    } else {
        log_message.push_str("No operator vote found\n");
    }

    log_message.push_str(&format!(
        "Consensus Reached: {}\n",
        ballot_box.is_consensus_reached()
    ));

    if ballot_box.is_consensus_reached() {
        log_message.push_str(&format!(
            "Winning Ballot: {}\n",
            ballot_box.get_winning_ballot()?
        ));
    }
    log_message.push_str("--------------------------\n");

    log::info!("{}", log_message);

    Ok(())
}

// --------------------- MIDDLEWARE ------------------------------

pub async fn get_consensus_result_instruction(handler: &CliHandler, epoch: u64) -> Result<()> {
    let consensus_result = get_consensus_result(handler, epoch).await?;

    info!(
        "Consensus Result for epoch {}: {:?}",
        epoch, consensus_result
    );

    Ok(())
}

pub const CREATE_TIMEOUT_MS: u64 = 2000;
pub const CREATE_GET_RETRIES: u64 = 3;
pub async fn check_created(handler: &CliHandler, address: &Pubkey) -> Result<()> {
    let mut retries = 0;
    let mut account = get_account(handler, address).await?;
    while account.is_none() && retries < CREATE_GET_RETRIES {
        sleep(Duration::from_millis(CREATE_TIMEOUT_MS * (retries + 1))).await;
        account = get_account(handler, address).await?;
        retries += 1;
    }

    if account.is_none() {
        return Err(anyhow!(
            "Failed to get account after creation {:?}",
            address
        ));
    }

    Ok(())
}

// --------------------- HELPERS -------------------------

pub async fn send_and_log_transaction(
    handler: &CliHandler,
    instructions: &[Instruction],
    signing_keypairs: &[&Keypair],
    title: &str,
    log_items: &[String],
) -> Result<()> {
    sleep(Duration::from_secs(1)).await;

    let signature = send_transactions(handler, instructions, signing_keypairs).await?;

    log_transaction(title, signature, log_items);

    Ok(())
}

pub async fn send_transactions(
    handler: &CliHandler,
    instructions: &[Instruction],
    signing_keypairs: &[&Keypair],
) -> Result<Signature> {
    let client = handler.rpc_client();
    let keypair = handler.keypair()?;
    let retries = handler.retries;
    let priority_fee_micro_lamports = handler.priority_fee_micro_lamports;

    let mut all_instructions = vec![];

    all_instructions.push(ComputeBudgetInstruction::set_compute_unit_price(
        priority_fee_micro_lamports,
    ));

    all_instructions.extend_from_slice(instructions);

    for iteration in 0..retries {
        let blockhash = client.get_latest_blockhash().await?;

        // Create a vector that combines all signing keypairs
        let mut all_signers = vec![keypair];
        all_signers.extend(signing_keypairs.iter());

        let tx = Transaction::new_signed_with_payer(
            &all_instructions,
            Some(&keypair.pubkey()),
            &all_signers, // Pass the reference to the vector of keypair references
            blockhash,
        );

        let config = RpcSendTransactionConfig {
            skip_preflight: true,
            ..RpcSendTransactionConfig::default()
        };
        let result = client
            .send_and_confirm_transaction_with_spinner_and_config(&tx, client.commitment(), config)
            .await;

        if result.is_err() {
            info!(
                "Retrying transaction after {}s {}/{}",
                (1 + iteration),
                iteration,
                retries
            );

            boring_progress_bar((1 + iteration) * 1000).await;
            continue;
        }

        return Ok(result?);
    }

    // last retry
    let blockhash = client.get_latest_blockhash().await?;

    // Create a vector that combines all signing keypairs
    let mut all_signers = vec![keypair];
    all_signers.extend(signing_keypairs.iter());

    let tx = Transaction::new_signed_with_payer(
        instructions,
        Some(&keypair.pubkey()),
        &all_signers, // Pass the reference to the vector of keypair references
        blockhash,
    );

    let result = client.send_and_confirm_transaction(&tx).await;

    if let Err(e) = result {
        return Err(anyhow!("\nError: \n\n{:?}\n\n", e));
    }

    Ok(result?)
}

pub fn log_transaction(title: &str, signature: Signature, log_items: &[String]) {
    let mut log_message = format!(
        "\n\n---------- {} ----------\nSignature: {:?}",
        title, signature
    );

    for item in log_items {
        log_message.push_str(&format!("\n{}", item));
    }

    // msg!(log_message.clone());

    log_message.push('\n');
    info!("{}", log_message);
}
