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
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
};

pub fn process_initialize_ballot_box(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    epoch: u64,
) -> ProgramResult {
    msg!("Processing initialize ballot box for epoch: {}", epoch);

    let account_info_iter = &mut accounts.iter();
    let epoch_marker = next_account_info(account_info_iter)?;
    let epoch_state = next_account_info(account_info_iter)?;
    let ncn_config = next_account_info(account_info_iter)?;
    let ballot_box = next_account_info(account_info_iter)?;
    let ncn = next_account_info(account_info_iter)?;
    let account_payer = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    let consensus_result = next_account_info(account_info_iter)?;

    msg!("Verifying accounts");
    msg!("Checking ballot box is a owned by the system program");
    load_system_account(ballot_box, true)?;

    msg!("Checking system program");
    load_system_program(system_program)?;

    msg!("Loading NCN account: {}", ncn.key);
    Ncn::load(&jito_restaking_program::id(), ncn, false)?;

    msg!(
        "Loading and checking epoch state for closing: {}, epoch: {}",
        ncn.key,
        epoch
    );
    EpochState::load_and_check_is_closing(program_id, epoch_state, ncn.key, epoch, false)?;

    msg!("Loading NCN config: {}", ncn.key);
    NcnConfig::load(program_id, ncn_config, ncn.key, false)?;

    msg!("Loading account payer: {}", account_payer.key);
    AccountPayer::load(program_id, account_payer, ncn.key, true)?;

    msg!(
        "Checking epoch marker doesn't exist: {}, epoch: {}",
        ncn.key,
        epoch
    );
    EpochMarker::check_dne(program_id, epoch_marker, ncn.key, epoch)?;

    // Initialize ballot box account
    msg!("Finding program address for ballot box");
    let (ballot_box_pda, ballot_box_bump, mut ballot_box_seeds) =
        BallotBox::find_program_address(program_id, ncn.key, epoch);
    ballot_box_seeds.push(vec![ballot_box_bump]);

    msg!(
        "Generated ballot box PDA: {}, bump: {}",
        ballot_box_pda,
        ballot_box_bump
    );

    if ballot_box_pda != *ballot_box.key {
        msg!(
            "Error: Invalid ballot box PDA. Expected: {}, got: {}",
            ballot_box_pda,
            ballot_box.key
        );
        return Err(ProgramError::InvalidSeeds);
    }

    msg!(
        "Creating ballot box account with {} bytes",
        MAX_REALLOC_BYTES
    );
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
    msg!(
        "Ballot box account created successfully: {}",
        ballot_box.key
    );

    // Initialize consensus result account
    msg!("Checking consensus result account is a system account");
    load_system_account(consensus_result, true)?;

    msg!("Finding program address for consensus result");
    let (consensus_result_pda, consensus_result_bump, mut consensus_result_seeds) =
        ConsensusResult::find_program_address(program_id, ncn.key, epoch);
    consensus_result_seeds.push(vec![consensus_result_bump]);

    msg!(
        "Generated consensus result PDA: {}, bump: {}",
        consensus_result_pda,
        consensus_result_bump
    );

    if consensus_result_pda != *consensus_result.key {
        msg!(
            "Error: Invalid consensus result PDA. Expected: {}, got: {}",
            consensus_result_pda,
            consensus_result.key
        );
        return Err(ProgramError::InvalidSeeds);
    }

    // Create consensus result account if it doesn't exist
    if consensus_result.data_is_empty() {
        let space = ConsensusResult::SIZE;
        msg!(
            "Consensus result account is empty, creating new account with {} bytes",
            space
        );

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
        msg!(
            "Consensus result account created successfully: {}",
            consensus_result.key
        );

        // Initialize the consensus result with discriminator
        msg!("Initializing consensus result account with discriminator");
        let mut consensus_result_data = consensus_result.try_borrow_mut_data()?;
        consensus_result_data[0] = ConsensusResult::DISCRIMINATOR;

        let consensus_result_account =
            ConsensusResult::try_from_slice_unchecked_mut(&mut consensus_result_data)?;
        consensus_result_account.initialize(ncn.key, epoch, consensus_result_bump)?;
        msg!("Consensus result account initialized successfully");
    } else {
        msg!(
            "Consensus result account already exists: {}",
            consensus_result.key
        );
    }

    msg!(
        "Ballot box initialization completed successfully for epoch: {}",
        epoch
    );
    Ok(())
}
