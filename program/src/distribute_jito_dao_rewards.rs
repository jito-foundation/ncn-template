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

pub fn process_distribute_jito_dao_rewards(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    epoch: u64,
) -> ProgramResult {
    let [epoch_state, ncn_config, ncn, ncn_reward_router, ncn_reward_receiver, jito_dao_fee_wallet, system_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    EpochState::load(program_id, epoch_state, ncn.key, epoch, true)?;
    Ncn::load(&jito_restaking_program::id(), ncn, false)?;
    Config::load(program_id, ncn_config, ncn.key, false)?;
    NCNRewardRouter::load(program_id, ncn_reward_router, ncn.key, epoch, true)?;
    NCNRewardReceiver::load(program_id, ncn_reward_receiver, ncn.key, epoch, true)?;

    {
        let ncn_config_data = ncn_config.try_borrow_data()?;
        let ncn_config_account = Config::try_from_slice_unchecked(&ncn_config_data)?;
        let fee_wallet = ncn_config_account.fee_config.jito_dao_fee_wallet();

        if fee_wallet.ne(jito_dao_fee_wallet.key) {
            msg!("Incorrect Jito DAO fee wallet");
            return Err(ProgramError::InvalidAccountData);
        }
    }

    // Get rewards and update state
    let rewards = {
        let mut ncn_reward_router_data = ncn_reward_router.try_borrow_mut_data()?;
        let ncn_reward_router_account =
            NCNRewardRouter::try_from_slice_unchecked_mut(&mut ncn_reward_router_data)?;

        if ncn_reward_router_account.still_routing() {
            msg!("Rewards still routing");
            return Err(NCNProgramError::RouterStillRouting.into());
        }

        ncn_reward_router_account.distribute_jito_dao_fee_rewards()?
    };

    if rewards > 0 {
        let (_, ncn_reward_receiver_bump, mut ncn_reward_receiver_seeds) =
            NCNRewardReceiver::find_program_address(program_id, ncn.key, epoch);
        ncn_reward_receiver_seeds.push(vec![ncn_reward_receiver_bump]);

        let ncn_reward_receiver_balance = **ncn_reward_receiver.try_borrow_lamports()?;
        msg!(
            "NCN reward receiver balance: {} lamports",
            ncn_reward_receiver_balance
        );

        // Transfer rewards from receiver to Jito DAO fee wallet
        let transfer_instruction =
            system_instruction::transfer(ncn_reward_receiver.key, jito_dao_fee_wallet.key, rewards);

        invoke_signed(
            &transfer_instruction,
            &[
                ncn_reward_receiver.clone(),
                jito_dao_fee_wallet.clone(),
                system_program.clone(),
            ],
            &[ncn_reward_receiver_seeds
                .iter()
                .map(|s| s.as_slice())
                .collect::<Vec<&[u8]>>()
                .as_slice()],
        )?;

        msg!("Transferred {} lamports to Jito DAO fee wallet", rewards);
    }

    {
        let mut epoch_state_data = epoch_state.try_borrow_mut_data()?;
        let epoch_state_account = EpochState::try_from_slice_unchecked_mut(&mut epoch_state_data)?;
        epoch_state_account.update_distribute_jito_dao_rewards(rewards);
        msg!(
            "Updated epoch state with distributed Jito DAO rewards: {} lamports",
            rewards
        );
    }

    Ok(())
}
