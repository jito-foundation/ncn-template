use jito_bytemuck::{AccountDeserialize, Discriminator};
use jito_jsm_core::loader::load_system_program;
use jito_restaking_core::ncn::Ncn;
use ncn_program_core::{
    account_payer::AccountPayer, config::Config as NcnConfig, epoch_state::EpochState,
    ncn_reward_router::NCNRewardRouter, utils::get_new_size,
};
use solana_program::{
    account_info::AccountInfo, clock::Clock, entrypoint::ProgramResult, msg,
    program_error::ProgramError, pubkey::Pubkey, sysvar::Sysvar,
};

pub fn process_realloc_ncn_reward_router(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    epoch: u64,
) -> ProgramResult {
    msg!(
        "Starting NCN reward router reallocation for epoch {}",
        epoch
    );

    let [epoch_state, ncn_config, ncn_reward_router, ncn, account_payer, system_program] = accounts
    else {
        msg!("Error: Invalid number of accounts provided");
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    msg!("Loading system program...");
    load_system_program(system_program)?;

    msg!("Loading NCN account...");
    Ncn::load(&jito_restaking_program::id(), ncn, false)?;

    msg!("Loading epoch state...");
    EpochState::load(program_id, epoch_state, ncn.key, epoch, true)?;

    msg!("Loading NCN config...");
    NcnConfig::load(program_id, ncn_config, ncn.key, false)?;

    msg!("Loading account payer...");
    AccountPayer::load(program_id, account_payer, ncn.key, true)?;

    let (ncn_reward_router_pda, ncn_reward_router_bump, _) =
        NCNRewardRouter::find_program_address(program_id, ncn.key, epoch);

    msg!("Expected NCN reward router PDA: {}", ncn_reward_router_pda);
    msg!("Actual NCN reward router key: {}", ncn_reward_router.key);

    if ncn_reward_router_pda != *ncn_reward_router.key {
        msg!("Error: NCN reward router account is not at the correct PDA");
        return Err(ProgramError::InvalidAccountData);
    }

    msg!(
        "Current NCN reward router size: {} bytes",
        ncn_reward_router.data_len()
    );
    msg!(
        "Required NCN reward router size: {} bytes",
        NCNRewardRouter::SIZE
    );

    if ncn_reward_router.data_len() < NCNRewardRouter::SIZE {
        let new_size = get_new_size(ncn_reward_router.data_len(), NCNRewardRouter::SIZE)?;
        msg!(
            "Reallocating NCN reward router from {} bytes to {} bytes",
            ncn_reward_router.data_len(),
            new_size
        );
        AccountPayer::pay_and_realloc(
            program_id,
            ncn.key,
            account_payer,
            ncn_reward_router,
            new_size,
        )?;
        msg!("NCN reward router reallocation completed successfully");
    } else {
        msg!("NCN reward router size is sufficient, no reallocation needed");
    }

    let should_initialize = ncn_reward_router.data_len() >= NCNRewardRouter::SIZE
        && ncn_reward_router.try_borrow_data()?[0] != NCNRewardRouter::DISCRIMINATOR;

    if should_initialize {
        msg!("Initializing NCN reward router account...");
        let mut ncn_reward_router_data = ncn_reward_router.try_borrow_mut_data()?;
        ncn_reward_router_data[0] = NCNRewardRouter::DISCRIMINATOR;
        let ncn_reward_router_account =
            NCNRewardRouter::try_from_slice_unchecked_mut(&mut ncn_reward_router_data)?;

        ncn_reward_router_account.initialize(
            ncn.key,
            epoch,
            ncn_reward_router_bump,
            Clock::get()?.slot,
        );
        msg!("NCN reward router initialized successfully");

        // Update Epoch State
        msg!("Updating epoch state...");
        {
            let mut epoch_state_data = epoch_state.try_borrow_mut_data()?;
            let epoch_state_account =
                EpochState::try_from_slice_unchecked_mut(&mut epoch_state_data)?;
            epoch_state_account.update_realloc_ncn_reward_router();
        }
        msg!("Epoch state updated successfully");
    } else {
        msg!("NCN reward router already initialized, skipping initialization");
    }

    msg!("NCN reward router reallocation process completed successfully");
    Ok(())
}
