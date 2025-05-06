use jito_bytemuck::{AccountDeserialize, Discriminator};
use jito_jsm_core::loader::{load_system_account, load_system_program};
use jito_restaking_core::ncn::Ncn;
use ncn_program_core::{
    account_payer::AccountPayer, ballot_box::BallotBox, config::Config as NcnConfig,
    consensus_result::ConsensusResult, constants::MAX_REALLOC_BYTES, epoch_marker::EpochMarker,
    epoch_state::EpochState,
};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    program_error::ProgramError,
    pubkey::Pubkey,
};

pub fn process_initialize_ballot_box(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    epoch: u64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let epoch_marker = next_account_info(account_info_iter)?;
    let epoch_state = next_account_info(account_info_iter)?;
    let ncn_config = next_account_info(account_info_iter)?;
    let ballot_box = next_account_info(account_info_iter)?;
    let ncn = next_account_info(account_info_iter)?;
    let account_payer = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    let consensus_result = next_account_info(account_info_iter)?;

    // Verify accounts
    load_system_account(ballot_box, true)?;
    load_system_program(system_program)?;

    Ncn::load(&jito_restaking_program::id(), ncn, false)?;
    EpochState::load_and_check_is_closing(program_id, epoch_state, ncn.key, epoch, false)?;
    NcnConfig::load(program_id, ncn_config, ncn.key, false)?;
    AccountPayer::load(program_id, account_payer, ncn.key, true)?;
    EpochMarker::check_dne(program_id, epoch_marker, ncn.key, epoch)?;

    // Initialize ballot box account
    let (ballot_box_pda, ballot_box_bump, mut ballot_box_seeds) =
        BallotBox::find_program_address(program_id, ncn.key, epoch);
    ballot_box_seeds.push(vec![ballot_box_bump]);

    if ballot_box_pda != *ballot_box.key {
        return Err(ProgramError::InvalidSeeds);
    }

    AccountPayer::pay_and_create_account(
        program_id,
        ncn.key,
        account_payer,
        ballot_box,
        system_program,
        program_id,
        MAX_REALLOC_BYTES as usize,
        &ballot_box_seeds,
    )?;

    // Initialize consensus result account
    load_system_account(consensus_result, true)?;

    let (consensus_result_pda, consensus_result_bump, mut consensus_result_seeds) =
        ConsensusResult::find_program_address(program_id, ncn.key, epoch);
    consensus_result_seeds.push(vec![consensus_result_bump]);

    if consensus_result_pda != *consensus_result.key {
        return Err(ProgramError::InvalidSeeds);
    }

    // Create consensus result account if it doesn't exist
    if consensus_result.data_is_empty() {
        let space = ConsensusResult::SIZE;

        AccountPayer::pay_and_create_account(
            program_id,
            ncn.key,
            account_payer,
            consensus_result,
            system_program,
            program_id,
            space,
            &consensus_result_seeds,
        )?;

        // Initialize the consensus result with discriminator
        let mut consensus_result_data = consensus_result.try_borrow_mut_data()?;
        consensus_result_data[0] = ConsensusResult::DISCRIMINATOR;

        let consensus_result_account =
            ConsensusResult::try_from_slice_unchecked_mut(&mut consensus_result_data)?;
        consensus_result_account.initialize(ncn.key, epoch, consensus_result_bump)?;
    }

    Ok(())
}
