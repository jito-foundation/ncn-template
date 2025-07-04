use jito_bytemuck::{AccountDeserialize, Discriminator};
use jito_jsm_core::loader::{load_system_account, load_system_program};
use jito_restaking_core::{ncn::Ncn, operator::Operator};
use ncn_program_core::{
    account_payer::AccountPayer,
    epoch_marker::EpochMarker,
    epoch_snapshot::OperatorSnapshot,
    epoch_state::EpochState,
    operator_vault_reward_router::{OperatorVaultRewardReceiver, OperatorVaultRewardRouter},
};
use solana_program::{
    account_info::AccountInfo, clock::Clock, entrypoint::ProgramResult, msg,
    program_error::ProgramError, pubkey::Pubkey, rent::Rent, sysvar::Sysvar,
};

/// Can be backfilled for previous epochs
pub fn process_initialize_operator_vault_reward_router(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    epoch: u64,
) -> ProgramResult {
    let [epoch_marker, epoch_state, ncn, operator, operator_snapshot, operator_vault_reward_router, operator_vault_reward_receiver, account_payer, system_program] =
        accounts
    else {
        msg!("Error: Not enough account keys provided");
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    EpochState::load_and_check_is_closing(program_id, epoch_state, ncn.key, epoch, true)?;
    Ncn::load(&jito_restaking_program::id(), ncn, false)?;
    Operator::load(&jito_restaking_program::id(), operator, false)?;
    OperatorSnapshot::load(
        program_id,
        operator_snapshot,
        operator.key,
        ncn.key,
        epoch,
        false,
    )?;
    OperatorVaultRewardReceiver::load(
        program_id,
        operator_vault_reward_receiver,
        operator.key,
        ncn.key,
        epoch,
        true,
    )?;

    load_system_account(operator_vault_reward_router, true)?;
    load_system_program(system_program)?;
    AccountPayer::load(program_id, account_payer, ncn.key, true)?;
    EpochMarker::check_dne(program_id, epoch_marker, ncn.key, epoch)?;

    let operator_ncn_index = {
        let operator_snapshot_data = operator_snapshot.try_borrow_data()?;
        let operator_snapshot_account =
            OperatorSnapshot::try_from_slice_unchecked(&operator_snapshot_data)?;
        operator_snapshot_account.ncn_operator_index()
    };

    let current_slot = Clock::get()?.slot;

    let (
        operator_vault_reward_router_pubkey,
        operator_vault_reward_router_bump,
        mut operator_vault_reward_router_seeds,
    ) = OperatorVaultRewardRouter::find_program_address(program_id, operator.key, ncn.key, epoch);
    operator_vault_reward_router_seeds.push(vec![operator_vault_reward_router_bump]);

    if operator_vault_reward_router_pubkey.ne(operator_vault_reward_router.key) {
        msg!(
            "Error: Incorrect NCN reward router PDA. Expected: {}, Got: {}",
            operator_vault_reward_router_pubkey,
            operator_vault_reward_router.key
        );
        return Err(ProgramError::InvalidAccountData);
    }

    AccountPayer::pay_and_create_account(
        program_id,
        ncn.key,
        account_payer,
        operator_vault_reward_router,
        system_program,
        program_id,
        OperatorVaultRewardRouter::SIZE,
        &operator_vault_reward_router_seeds,
    )?;

    let mut operator_vault_reward_router_data =
        operator_vault_reward_router.try_borrow_mut_data()?;
    operator_vault_reward_router_data[0] = OperatorVaultRewardRouter::DISCRIMINATOR;
    let operator_vault_reward_router_account =
        OperatorVaultRewardRouter::try_from_slice_unchecked_mut(
            &mut operator_vault_reward_router_data,
        )?;

    *operator_vault_reward_router_account = OperatorVaultRewardRouter::new(
        operator.key,
        operator_ncn_index,
        ncn.key,
        epoch,
        operator_vault_reward_router_bump,
        current_slot,
    );

    let min_rent = Rent::get()?.minimum_balance(0);
    msg!(
        "Transferring minimum rent of {} lamports to reward receiver {}",
        min_rent,
        operator_vault_reward_receiver.key
    );
    AccountPayer::transfer(
        program_id,
        ncn.key,
        account_payer,
        operator_vault_reward_receiver,
        min_rent,
    )?;

    {
        let mut epoch_state_data = epoch_state.try_borrow_mut_data()?;
        let epoch_state_account = EpochState::try_from_slice_unchecked_mut(&mut epoch_state_data)?;
        epoch_state_account
            .update_realloc_operator_vault_reward_router(operator_ncn_index as usize);
    }

    Ok(())
}
