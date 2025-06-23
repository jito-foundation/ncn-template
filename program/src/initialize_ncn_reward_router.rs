use jito_jsm_core::loader::{load_system_account, load_system_program};
use jito_restaking_core::ncn::Ncn;
use ncn_program_core::{
    account_payer::AccountPayer,
    constants::MAX_REALLOC_BYTES,
    epoch_marker::EpochMarker,
    epoch_state::EpochState,
    ncn_reward_router::{NCNRewardReceiver, NCNRewardRouter},
};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, msg, program_error::ProgramError,
    pubkey::Pubkey, rent::Rent, sysvar::Sysvar,
};

/// Can be backfilled for previous epochs
pub fn process_initialize_ncn_reward_router(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    epoch: u64,
) -> ProgramResult {
    msg!(
        "Starting NCN reward router initialization for epoch: {}",
        epoch
    );

    let [epoch_marker, epoch_state, ncn, ncn_reward_router, ncn_reward_receiver, account_payer, system_program] =
        accounts
    else {
        msg!("Error: Expected 7 accounts but received {}", accounts.len());
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    msg!("Loading and validating epoch state for epoch: {}", epoch);
    EpochState::load_and_check_is_closing(program_id, epoch_state, ncn.key, epoch, false)?;

    msg!("Loading NCN account: {}", ncn.key);
    Ncn::load(&jito_restaking_program::id(), ncn, false)?;

    msg!(
        "Loading NCN reward receiver account: {}",
        ncn_reward_receiver.key
    );
    NCNRewardReceiver::load(program_id, ncn_reward_receiver, ncn.key, epoch, true)?;

    msg!("Loading account payer: {}", account_payer.key);
    AccountPayer::load(program_id, account_payer, ncn.key, true)?;

    msg!("Checking epoch marker does not exist for epoch: {}", epoch);
    EpochMarker::check_dne(program_id, epoch_marker, ncn.key, epoch)?;

    msg!("Loading system accounts");
    load_system_account(ncn_reward_router, true)?;
    load_system_program(system_program)?;

    msg!(
        "Deriving NCN reward router PDA for NCN: {} and epoch: {}",
        ncn.key,
        epoch
    );
    let (ncn_reward_router_pubkey, ncn_reward_router_bump, mut ncn_reward_router_seeds) =
        NCNRewardRouter::find_program_address(program_id, ncn.key, epoch);
    ncn_reward_router_seeds.push(vec![ncn_reward_router_bump]);
    msg!(
        "Derived PDA: {} with bump: {}",
        ncn_reward_router_pubkey,
        ncn_reward_router_bump
    );

    if ncn_reward_router_pubkey.ne(ncn_reward_router.key) {
        msg!("Error: Incorrect NCN reward router PDA");
        return Err(ProgramError::InvalidAccountData);
    }

    msg!(
        "Creating NCN Reward Router account {} for NCN: {} at epoch: {}",
        ncn_reward_router.key,
        ncn.key,
        epoch
    );
    AccountPayer::pay_and_create_account(
        program_id,
        ncn.key,
        account_payer,
        ncn_reward_router,
        system_program,
        program_id,
        MAX_REALLOC_BYTES as usize,
        &ncn_reward_router_seeds,
    )?;

    let min_rent = Rent::get()?.minimum_balance(0);
    msg!(
        "Transferring rent of {} lamports to NCN reward receiver {}",
        min_rent,
        ncn_reward_receiver.key
    );
    AccountPayer::transfer(
        program_id,
        ncn.key,
        account_payer,
        ncn_reward_receiver,
        min_rent,
    )?;

    Ok(())
}
