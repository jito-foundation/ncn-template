use jito_bytemuck::AccountDeserialize;
use jito_restaking_core::{ncn::Ncn, operator::Operator};
use ncn_program_core::{
    epoch_snapshot::OperatorSnapshot,
    epoch_state::EpochState,
    operator_vault_reward_router::{OperatorVaultRewardReceiver, OperatorVaultRewardRouter},
};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, msg, program_error::ProgramError,
    pubkey::Pubkey, rent::Rent, sysvar::Sysvar,
};

/// Can be backfilled for previous epochs
pub fn process_route_operator_vault_rewards(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    max_iterations: u16,
    epoch: u64,
) -> ProgramResult {
    msg!("Starting route_operator_vault_rewards process");
    msg!("Processing epoch: {}", epoch);
    msg!("Max iterations: {}", max_iterations);

    let [epoch_state, ncn, operator, operator_snapshot, ncn_reward_router, ncn_reward_receiver] =
        accounts
    else {
        msg!("Error: Not enough account keys provided");
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    msg!("Loading accounts...");

    EpochState::load(program_id, epoch_state, ncn.key, epoch, true)?;
    msg!("✓ EpochState loaded successfully");

    Ncn::load(&jito_restaking_program::id(), ncn, false)?;
    msg!("✓ NCN account loaded successfully");

    Operator::load(&jito_restaking_program::id(), operator, false)?;
    msg!("✓ Operator account loaded successfully");

    OperatorVaultRewardReceiver::load(
        program_id,
        ncn_reward_receiver,
        operator.key,
        ncn.key,
        epoch,
        true,
    )?;
    msg!("✓ OperatorVaultRewardReceiver loaded successfully");

    OperatorSnapshot::load(
        program_id,
        operator_snapshot,
        operator.key,
        ncn.key,
        epoch,
        false,
    )?;
    msg!("✓ OperatorSnapshot loaded successfully");

    OperatorVaultRewardRouter::load(
        program_id,
        ncn_reward_router,
        operator.key,
        ncn.key,
        epoch,
        true,
    )?;
    msg!("✓ OperatorVaultRewardRouter loaded successfully");

    let operator_snapshot_data = operator_snapshot.try_borrow_data()?;
    let operator_snapshot_account =
        OperatorSnapshot::try_from_slice_unchecked(&operator_snapshot_data)?;
    msg!("✓ OperatorSnapshot data deserialized");

    let account_balance = **ncn_reward_receiver.try_borrow_lamports()?;
    msg!("Account balance: {} lamports", account_balance);

    let mut ncn_reward_router_data = ncn_reward_router.try_borrow_mut_data()?;
    let ncn_reward_router_account =
        OperatorVaultRewardRouter::try_from_slice_unchecked_mut(&mut ncn_reward_router_data)?;
    msg!("✓ OperatorVaultRewardRouter data deserialized");

    let rent_cost = Rent::get()?.minimum_balance(0);
    msg!("Rent cost: {} lamports", rent_cost);

    if !ncn_reward_router_account.still_routing() {
        msg!("Routing is not in progress, starting new routing process");
        ncn_reward_router_account.route_incoming_rewards(rent_cost, account_balance)?;
        msg!("✓ Incoming rewards routed");
        ncn_reward_router_account.route_operator_rewards(operator_snapshot_account)?;
        msg!("✓ Operator rewards routed");
    } else {
        msg!("Routing already in progress, continuing existing process");
    }

    msg!("Routing reward pool...");
    ncn_reward_router_account.route_reward_pool(operator_snapshot_account, max_iterations)?;
    msg!("✓ Reward pool routing completed");

    {
        let mut epoch_state_data = epoch_state.try_borrow_mut_data()?;
        let epoch_state_account = EpochState::try_from_slice_unchecked_mut(&mut epoch_state_data)?;
        let ncn_operator_index = operator_snapshot_account.ncn_operator_index() as usize;
        let total_rewards = ncn_reward_router_account.total_rewards();

        msg!(
            "Updating epoch state - NCN operator index: {}, total rewards: {}",
            ncn_operator_index,
            total_rewards
        );

        epoch_state_account.update_route_operator_vault_rewards(ncn_operator_index, total_rewards);
        msg!("✓ Epoch state updated successfully");
    }

    msg!("route_operator_vault_rewards process completed successfully");
    Ok(())
}
