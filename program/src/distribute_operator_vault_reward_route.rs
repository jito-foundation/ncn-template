use jito_bytemuck::AccountDeserialize;
use jito_jsm_core::loader::load_system_program;
use jito_restaking_core::{ncn::Ncn, operator::Operator};
use ncn_program_core::{
    config::Config as NcnConfig,
    epoch_state::EpochState,
    error::NCNProgramError,
    ncn_reward_router::{NCNRewardReceiver, NCNRewardRouter},
    operator_vault_reward_router::{OperatorVaultRewardReceiver, OperatorVaultRewardRouter},
};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, msg, program_error::ProgramError,
    pubkey::Pubkey,
};

/// Can be backfilled for previous epochs
pub fn process_distribute_operator_vault_reward_route(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    epoch: u64,
) -> ProgramResult {
    msg!(
        "Starting operator vault reward route distribution for epoch {}",
        epoch
    );

    let [epoch_state, ncn_config, ncn, operator, ncn_reward_router, ncn_reward_receiver, operator_vault_reward_router, operator_vault_reward_receiver, system_program] =
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
    msg!("Loading NCN config");
    NcnConfig::load(program_id, ncn_config, ncn.key, false)?;
    msg!("Loading NCN reward router");
    NCNRewardRouter::load(program_id, ncn_reward_router, ncn.key, epoch, true)?;
    msg!("Loading operator vault reward router");
    OperatorVaultRewardRouter::load(
        program_id,
        operator_vault_reward_router,
        operator.key,
        ncn.key,
        epoch,
        false,
    )?;
    msg!("Loading NCN reward receiver");
    NCNRewardReceiver::load(program_id, ncn_reward_receiver, ncn.key, epoch, true)?;
    msg!("Loading operator vault reward receiver");
    OperatorVaultRewardReceiver::load(
        program_id,
        operator_vault_reward_receiver,
        operator.key,
        ncn.key,
        epoch,
        false,
    )?;

    load_system_program(system_program)?;

    // Get rewards and update state
    msg!("Calculating operator vault reward route");
    let rewards = {
        let mut epoch_reward_router_data = ncn_reward_router.try_borrow_mut_data()?;
        let ncn_reward_router_account =
            NCNRewardRouter::try_from_slice_unchecked_mut(&mut epoch_reward_router_data)?;

        if ncn_reward_router_account.still_routing() {
            msg!("Error: Rewards still routing, cannot distribute yet");
            return Err(NCNProgramError::RouterStillRouting.into());
        }

        let rewards =
            ncn_reward_router_account.distribute_operator_vault_reward_route(operator.key)?;
        msg!(
            "Calculated operator vault reward route: {} lamports",
            rewards
        );
        rewards
    };

    // Send rewards
    if rewards > 0 {
        msg!(
            "Distributing {} lamports to operator vault reward receiver",
            rewards
        );

        let (_, ncn_reward_receiver_bump, mut ncn_reward_receiver_seeds) =
            NCNRewardReceiver::find_program_address(program_id, ncn.key, epoch);
        ncn_reward_receiver_seeds.push(vec![ncn_reward_receiver_bump]);

        solana_program::program::invoke_signed(
            &solana_program::system_instruction::transfer(
                ncn_reward_receiver.key,
                operator_vault_reward_receiver.key,
                rewards,
            ),
            &[
                ncn_reward_receiver.clone(),
                operator_vault_reward_receiver.clone(),
            ],
            &[ncn_reward_receiver_seeds
                .iter()
                .map(|s| s.as_slice())
                .collect::<Vec<&[u8]>>()
                .as_slice()],
        )?;

        msg!(
            "Successfully transferred {} lamports to operator vault reward receiver",
            rewards
        );
    } else {
        msg!("No rewards to distribute (0 lamports)");
    }

    {
        let mut epoch_state_data = epoch_state.try_borrow_mut_data()?;
        let epoch_state_account = EpochState::try_from_slice_unchecked_mut(&mut epoch_state_data)?;
        epoch_state_account.update_distribute_operator_vault_rewards(rewards);
        msg!(
            "Updated epoch state with distributed operator vault rewards: {} lamports",
            rewards
        );
    }

    msg!(
        "Operator vault reward route distribution completed successfully for epoch {}",
        epoch
    );
    Ok(())
}
