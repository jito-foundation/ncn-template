use jito_bytemuck::AccountDeserialize;
use jito_restaking_core::ncn::Ncn;
use ncn_program_core::{
    config::Config,
    epoch_state::EpochState,
    error::NCNProgramError,
    ncn_reward_router::{NCNRewardReceiver, NCNRewardRouter},
};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, msg, program::invoke_signed,
    program_error::ProgramError, pubkey::Pubkey, system_instruction,
};

pub fn process_distribute_protocol_rewards(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    epoch: u64,
) -> ProgramResult {
    msg!("Starting Protocol rewards distribution for epoch {}", epoch);

    let [epoch_state, ncn_config, ncn, ncn_reward_router, ncn_reward_receiver, protocol_fee_wallet, system_program] =
        accounts
    else {
        msg!("Error: Not enough account keys provided");
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    msg!("Loading epoch state for epoch {}", epoch);
    EpochState::load(program_id, epoch_state, ncn.key, epoch, true)?;
    msg!("Loading NCN account");
    Ncn::load(&jito_restaking_program::id(), ncn, false)?;
    msg!("Loading NCN config");
    Config::load(program_id, ncn_config, ncn.key, false)?;
    msg!("Loading NCN reward router");
    NCNRewardRouter::load(program_id, ncn_reward_router, ncn.key, epoch, true)?;
    msg!("Loading NCN reward receiver");
    NCNRewardReceiver::load(program_id, ncn_reward_receiver, ncn.key, epoch, true)?;

    {
        let ncn_config_data = ncn_config.try_borrow_data()?;
        let ncn_config_account = Config::try_from_slice_unchecked(&ncn_config_data)?;
        let fee_wallet = ncn_config_account.fee_config.protocol_fee_wallet();

        if fee_wallet.ne(protocol_fee_wallet.key) {
            msg!("Error: Incorrect Protocol fee wallet provided");
            return Err(ProgramError::InvalidAccountData);
        }
        msg!("Protocol fee wallet validation passed");
    }

    // Get rewards and update state
    msg!("Calculating protocol rewards");
    let rewards = {
        let mut ncn_reward_router_data = ncn_reward_router.try_borrow_mut_data()?;
        let ncn_reward_router_account =
            NCNRewardRouter::try_from_slice_unchecked_mut(&mut ncn_reward_router_data)?;

        if ncn_reward_router_account.still_routing() {
            msg!("Error: Rewards still routing, cannot distribute yet");
            return Err(NCNProgramError::RouterStillRouting.into());
        }

        let rewards = ncn_reward_router_account.distribute_protocol_fee_rewards()?;
        msg!("Calculated Protocol fee rewards: {} lamports", rewards);
        rewards
    };

    if rewards > 0 {
        msg!("Distributing {} lamports to Protocol fee wallet", rewards);

        let (_, ncn_reward_receiver_bump, mut ncn_reward_receiver_seeds) =
            NCNRewardReceiver::find_program_address(program_id, ncn.key, epoch);
        ncn_reward_receiver_seeds.push(vec![ncn_reward_receiver_bump]);

        let ncn_reward_receiver_balance = **ncn_reward_receiver.try_borrow_lamports()?;
        msg!(
            "NCN reward receiver balance: {} lamports",
            ncn_reward_receiver_balance
        );

        // Transfer rewards from receiver to Protocol fee wallet
        let transfer_instruction =
            system_instruction::transfer(ncn_reward_receiver.key, protocol_fee_wallet.key, rewards);

        invoke_signed(
            &transfer_instruction,
            &[
                ncn_reward_receiver.clone(),
                protocol_fee_wallet.clone(),
                system_program.clone(),
            ],
            &[ncn_reward_receiver_seeds
                .iter()
                .map(|s| s.as_slice())
                .collect::<Vec<&[u8]>>()
                .as_slice()],
        )?;

        msg!(
            "Successfully transferred {} lamports to Protocol fee wallet",
            rewards
        );
    } else {
        msg!("No rewards to distribute (0 lamports)");
    }

    msg!("Updating epoch state with distributed protocol rewards");
    {
        let mut epoch_state_data = epoch_state.try_borrow_mut_data()?;
        let epoch_state_account = EpochState::try_from_slice_unchecked_mut(&mut epoch_state_data)?;
        epoch_state_account.update_distribute_protocol_rewards(rewards);
        msg!(
            "Updated epoch state with distributed Protocol rewards: {} lamports",
            rewards
        );
    }

    msg!(
        "Protocol rewards distribution completed successfully for epoch {}",
        epoch
    );
    Ok(())
}
