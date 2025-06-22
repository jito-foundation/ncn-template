use jito_bytemuck::AccountDeserialize;
use jito_restaking_core::{ncn::Ncn, operator::Operator};
use jito_vault_core::vault::Vault;
use ncn_program_core::{
    config::Config as NcnConfig,
    epoch_snapshot::OperatorSnapshot,
    epoch_state::EpochState,
    error::NCNProgramError,
    operator_vault_reward_router::{OperatorVaultRewardReceiver, OperatorVaultRewardRouter},
};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, msg, program::invoke_signed,
    program_error::ProgramError, pubkey::Pubkey, system_instruction,
};

/// Can be backfilled for previous epochs
pub fn process_distribute_vault_rewards(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    epoch: u64,
) -> ProgramResult {
    msg!("Starting vault rewards distribution for epoch {}", epoch);

    let [epoch_state, ncn_config, ncn, operator, vault, operator_snapshot, operator_vault_reward_router, operator_vault_reward_receiver, system_program] =
        accounts
    else {
        msg!("Error: Not enough account keys provided");
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    msg!("Loading epoch state for epoch {}", epoch);
    EpochState::load(program_id, epoch_state, ncn.key, epoch, true)?;

    msg!("Loading NCN account");
    Ncn::load(&jito_restaking_program::id(), ncn, false)?;

    msg!("Loading operator account");
    Operator::load(&jito_restaking_program::id(), operator, false)?;

    msg!("Loading vault account");
    Vault::load(&jito_vault_program::id(), vault, true)?;

    msg!(
        "Loading operator snapshot for operator {} in epoch {}",
        operator.key,
        epoch
    );
    OperatorSnapshot::load(
        program_id,
        operator_snapshot,
        operator.key,
        ncn.key,
        epoch,
        true,
    )?;

    msg!("Loading NCN config");
    NcnConfig::load(program_id, ncn_config, ncn.key, false)?;

    msg!("Loading operator vault reward router");
    OperatorVaultRewardRouter::load(
        program_id,
        operator_vault_reward_router,
        operator.key,
        ncn.key,
        epoch,
        true,
    )?;

    msg!("Loading operator vault reward receiver");
    OperatorVaultRewardReceiver::load(
        program_id,
        operator_vault_reward_receiver,
        operator.key,
        ncn.key,
        epoch,
        true,
    )?;

    // Get rewards and update state
    msg!(
        "Calculating vault rewards for operator {} and vault {}",
        operator.key,
        vault.key
    );
    let rewards = {
        let mut operator_vault_reward_router_data =
            operator_vault_reward_router.try_borrow_mut_data()?;
        let operator_vault_reward_router_account =
            OperatorVaultRewardRouter::try_from_slice_unchecked_mut(
                &mut operator_vault_reward_router_data,
            )?;

        if operator_vault_reward_router_account.still_routing() {
            msg!("Error: Rewards still routing, cannot distribute yet");
            return Err(NCNProgramError::RouterStillRouting.into());
        }

        let calculated_rewards =
            operator_vault_reward_router_account.distribute_vault_reward_route(vault.key)?;
        msg!("Calculated vault rewards: {} lamports", calculated_rewards);
        calculated_rewards
    };

    if rewards > 0 {
        msg!(
            "Transferring {} lamports from operator vault reward receiver to vault",
            rewards
        );

        let (_, operator_vault_reward_receiver_bump, mut operator_vault_reward_receiver_seeds) =
            OperatorVaultRewardReceiver::find_program_address(
                program_id,
                operator.key,
                ncn.key,
                epoch,
            );

        operator_vault_reward_receiver_seeds.push(vec![operator_vault_reward_receiver_bump]);

        // Transfer rewards from receiver to NCN fee wallet
        let transfer_instruction =
            system_instruction::transfer(operator_vault_reward_receiver.key, vault.key, rewards);

        invoke_signed(
            &transfer_instruction,
            &[
                operator_vault_reward_receiver.clone(),
                vault.clone(),
                system_program.clone(),
            ],
            &[operator_vault_reward_receiver_seeds
                .iter()
                .map(|s| s.as_slice())
                .collect::<Vec<&[u8]>>()
                .as_slice()],
        )?;

        msg!(
            "Successfully transferred {} lamports to vault {}",
            rewards,
            vault.key
        );
    } else {
        msg!("No rewards to distribute (0 lamports)");
    }

    msg!("Updating epoch state with distributed vault rewards");
    {
        let operator_snapshot_data = operator_snapshot.try_borrow_data()?;
        let operator_snapshot_account =
            OperatorSnapshot::try_from_slice_unchecked(&operator_snapshot_data)?;

        let mut epoch_state_data = epoch_state.try_borrow_mut_data()?;
        let epoch_state_account = EpochState::try_from_slice_unchecked_mut(&mut epoch_state_data)?;
        epoch_state_account.update_distribute_operator_vault_route_rewards(
            operator_snapshot_account.ncn_operator_index() as usize,
            rewards,
        );
        msg!(
            "Updated epoch state with {} lamports distributed for operator index {}",
            rewards,
            operator_snapshot_account.ncn_operator_index()
        );
    }

    msg!(
        "Vault rewards distribution completed successfully for epoch {}",
        epoch
    );
    Ok(())
}
