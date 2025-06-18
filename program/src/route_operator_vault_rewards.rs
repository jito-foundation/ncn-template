use jito_bytemuck::AccountDeserialize;
use jito_restaking_core::{ncn::Ncn, operator::Operator};
use ncn_program_core::{
    epoch_snapshot::OperatorSnapshot,
    epoch_state::EpochState,
    operator_vault_reward_router::{OperatorVaultRewardReceiver, OperatorVaultRewardRouter},
};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, program_error::ProgramError,
    pubkey::Pubkey, rent::Rent, sysvar::Sysvar,
};

/// Can be backfilled for previous epochs
pub fn process_route_operator_vault_rewards(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    max_iterations: u16,
    epoch: u64,
) -> ProgramResult {
    let [epoch_state, ncn, operator, operator_snapshot, ncn_reward_router, ncn_reward_receiver] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    EpochState::load(program_id, epoch_state, ncn.key, epoch, true)?;
    Ncn::load(&jito_restaking_program::id(), ncn, false)?;
    Operator::load(&jito_restaking_program::id(), operator, false)?;
    OperatorVaultRewardReceiver::load(
        program_id,
        ncn_reward_receiver,
        operator.key,
        ncn.key,
        epoch,
        true,
    )?;

    OperatorSnapshot::load(
        program_id,
        operator_snapshot,
        operator.key,
        ncn.key,
        epoch,
        false,
    )?;
    OperatorVaultRewardRouter::load(
        program_id,
        ncn_reward_router,
        operator.key,
        ncn.key,
        epoch,
        true,
    )?;

    let operator_snapshot_data = operator_snapshot.try_borrow_data()?;
    let operator_snapshot_account =
        OperatorSnapshot::try_from_slice_unchecked(&operator_snapshot_data)?;

    let account_balance = **ncn_reward_receiver.try_borrow_lamports()?;

    let mut ncn_reward_router_data = ncn_reward_router.try_borrow_mut_data()?;
    let ncn_reward_router_account =
        OperatorVaultRewardRouter::try_from_slice_unchecked_mut(&mut ncn_reward_router_data)?;

    let rent_cost = Rent::get()?.minimum_balance(0);

    if !ncn_reward_router_account.still_routing() {
        ncn_reward_router_account.route_incoming_rewards(rent_cost, account_balance)?;
        ncn_reward_router_account.route_operator_rewards(operator_snapshot_account)?;
    }

    ncn_reward_router_account.route_reward_pool(operator_snapshot_account, max_iterations)?;

    {
        let mut epoch_state_data = epoch_state.try_borrow_mut_data()?;
        let epoch_state_account = EpochState::try_from_slice_unchecked_mut(&mut epoch_state_data)?;
        epoch_state_account.update_route_operator_vault_rewards(
            operator_snapshot_account.ncn_operator_index() as usize,
            ncn_reward_router_account.total_rewards(),
        );
    }

    Ok(())
}
