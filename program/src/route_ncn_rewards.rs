use jito_bytemuck::AccountDeserialize;
use jito_restaking_core::ncn::Ncn;
use ncn_program_core::{
    ballot_box::BallotBox,
    config::Config as NcnConfig,
    epoch_snapshot::EpochSnapshot,
    epoch_state::EpochState,
    error::NCNProgramError,
    ncn_reward_router::{NCNRewardReceiver, NCNRewardRouter},
};
use solana_program::{
    account_info::AccountInfo, clock::Clock, entrypoint::ProgramResult, msg,
    program_error::ProgramError, pubkey::Pubkey, rent::Rent, sysvar::Sysvar,
};

/// Can be backfilled for previous epochs
pub fn process_route_ncn_rewards(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    max_iterations: u16,
    epoch: u64,
) -> ProgramResult {
    msg!("Starting NCN reward routing for epoch: {}", epoch);
    msg!(
        "Max iterations for operator vault routing: {}",
        max_iterations
    );

    let [epoch_state, config, ncn, epoch_snapshot, ballot_box, ncn_reward_router, ncn_reward_receiver] =
        accounts
    else {
        msg!("Incorrect number of accounts");
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    msg!("Loading and verifying accounts...");
    EpochState::load(program_id, epoch_state, ncn.key, epoch, true)?;
    msg!("EpochState loaded successfully");

    NcnConfig::load(program_id, config, ncn.key, false)?;
    msg!("NCN Config loaded successfully");

    Ncn::load(&jito_restaking_program::id(), ncn, false)?;
    msg!("NCN account loaded successfully");

    EpochSnapshot::load(program_id, epoch_snapshot, ncn.key, epoch, false)?;
    msg!("EpochSnapshot loaded successfully");

    NCNRewardRouter::load(program_id, ncn_reward_router, ncn.key, epoch, true)?;
    msg!("NCNRewardRouter loaded successfully");

    BallotBox::load(program_id, ballot_box, ncn.key, epoch, false)?;
    msg!("BallotBox loaded successfully");

    NCNRewardReceiver::load(program_id, ncn_reward_receiver, ncn.key, epoch, true)?;
    msg!("NCNRewardReceiver loaded successfully");

    let epoch_snapshot_data = epoch_snapshot.try_borrow_data()?;
    let epoch_snapshot_account = EpochSnapshot::try_from_slice_unchecked(&epoch_snapshot_data)?;

    let ballot_box_data = ballot_box.try_borrow_data()?;
    let ballot_box_account = BallotBox::try_from_slice_unchecked(&ballot_box_data)?;

    msg!("Checking if voting is still ongoing...");
    let current_slot = Clock::get()?.slot;
    let valid_slots_after_consensus = {
        let ncn_config_data = config.data.borrow();
        let ncn_config = NcnConfig::try_from_slice_unchecked(&ncn_config_data)?;
        ncn_config.valid_slots_after_consensus()
    };

    // Do not route if voting is still ongoing
    if ballot_box_account.is_voting_valid(current_slot, valid_slots_after_consensus)? {
        msg!("Voting is still ongoing (current slot: {}, valid slots after consensus: {}), cannot route until voting is complete", current_slot, valid_slots_after_consensus);
        return Err(NCNProgramError::VotingIsNotOver.into());
    }
    msg!("Voting period has ended, proceeding with reward routing");

    let ncn_reward_receiver_balance = **ncn_reward_receiver.try_borrow_lamports()?;
    msg!(
        "NCN reward receiver balance: {} lamports",
        ncn_reward_receiver_balance
    );

    let mut ncn_reward_router_data = ncn_reward_router.try_borrow_mut_data()?;
    let ncn_reward_router_account =
        NCNRewardRouter::try_from_slice_unchecked_mut(&mut ncn_reward_router_data)?;

    let rent_cost = Rent::get()?.minimum_balance(0);
    msg!("Rent cost calculated: {} lamports", rent_cost);

    if !ncn_reward_router_account.still_routing() {
        msg!("NCN reward router is not still routing, processing incoming rewards and reward pool");

        ncn_reward_router_account.route_incoming_rewards(rent_cost, ncn_reward_receiver_balance)?;
        msg!("Incoming rewards routed successfully");

        let epoch_fees = epoch_snapshot_account.fees();
        ncn_reward_router_account.route_reward_pool(epoch_fees)?;
        msg!(
            "Reward pool routed successfully with epoch fees: {:?}",
            epoch_fees
        );
    } else {
        msg!(
            "NCN reward router is still routing, skipping incoming rewards and reward pool routing"
        );
    }

    msg!("Starting operator vault rewards routing...");
    ncn_reward_router_account.route_operator_vault_rewards(ballot_box_account, max_iterations)?;
    msg!("Operator vault rewards routing completed");

    let total_rewards = ncn_reward_router_account.total_rewards();
    msg!("Total rewards processed: {} lamports", total_rewards);

    let ncn_rewards = ncn_reward_router_account.ncn_rewards();
    let jito_rewards = ncn_reward_router_account.jito_dao_rewards();

    {
        msg!("Updating epoch state with total rewards...");
        let mut epoch_state_data = epoch_state.try_borrow_mut_data()?;
        let epoch_state_account = EpochState::try_from_slice_unchecked_mut(&mut epoch_state_data)?;
        epoch_state_account.update_route_ncn_rewards(ncn_rewards);
        epoch_state_account.update_route_jito_dao_rewards(jito_rewards);
        epoch_state_account.update_route_total_rewards(total_rewards);
        msg!("Epoch state updated successfully");
    }

    msg!(
        "NCN reward routing completed successfully for epoch: {}",
        epoch
    );
    Ok(())
}
