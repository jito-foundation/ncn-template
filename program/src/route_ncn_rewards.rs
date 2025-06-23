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
        msg!("ERROR: Incorrect number of accounts provided");
        msg!("Expected 7 accounts, got: {}", accounts.len());
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    msg!("Loading EpochState account: {}", epoch_state.key);
    EpochState::load(program_id, epoch_state, ncn.key, epoch, true)?;

    msg!("Loading NCN Config account: {}", config.key);
    NcnConfig::load(program_id, config, ncn.key, false)?;

    msg!("Loading NCN account: {}", ncn.key);
    Ncn::load(&jito_restaking_program::id(), ncn, false)?;

    msg!("Loading EpochSnapshot account: {}", epoch_snapshot.key);
    EpochSnapshot::load(program_id, epoch_snapshot, ncn.key, epoch, false)?;

    msg!("Loading NCNRewardRouter account: {}", ncn_reward_router.key);
    NCNRewardRouter::load(program_id, ncn_reward_router, ncn.key, epoch, true)?;

    msg!("Loading BallotBox account: {}", ballot_box.key);
    BallotBox::load(program_id, ballot_box, ncn.key, epoch, false)?;

    msg!(
        "Loading NCNRewardReceiver account: {}",
        ncn_reward_receiver.key
    );
    NCNRewardReceiver::load(program_id, ncn_reward_receiver, ncn.key, epoch, true)?;

    let epoch_snapshot_data = epoch_snapshot.try_borrow_data()?;
    let epoch_snapshot_account = EpochSnapshot::try_from_slice_unchecked(&epoch_snapshot_data)?;

    let ballot_box_data = ballot_box.try_borrow_data()?;
    let ballot_box_account = BallotBox::try_from_slice_unchecked(&ballot_box_data)?;

    msg!("Checking if voting period has ended");
    let current_slot = Clock::get()?.slot;
    msg!("Current slot: {}", current_slot);

    let valid_slots_after_consensus = {
        let ncn_config_data = config.data.borrow();
        let ncn_config = NcnConfig::try_from_slice_unchecked(&ncn_config_data)?;
        let valid_slots = ncn_config.valid_slots_after_consensus();
        msg!("Valid slots after consensus: {}", valid_slots);
        valid_slots
    };

    // Do not route if voting is still ongoing
    if ballot_box_account.is_voting_valid(current_slot, valid_slots_after_consensus)? {
        msg!("Voting is still ongoing - cannot route rewards yet");
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

    if !ncn_reward_router_account.still_routing() {
        msg!("Processing incoming rewards and reward pool");

        msg!("Routing incoming rewards");
        ncn_reward_router_account.route_incoming_rewards(rent_cost, ncn_reward_receiver_balance)?;

        let epoch_fees = epoch_snapshot_account.fees();
        msg!("Routing reward pool with epoch fees: {:?}", epoch_fees);
        ncn_reward_router_account.route_reward_pool(epoch_fees)?;
    } else {
        msg!("Skipping incoming rewards and reward pool routing since routing is already in progress");
    }

    msg!(
        "Routing operator vault rewards with max iterations: {}",
        max_iterations
    );
    ncn_reward_router_account.route_operator_vault_rewards(ballot_box_account, max_iterations)?;

    let total_rewards = ncn_reward_router_account.total_rewards();
    let ncn_rewards = ncn_reward_router_account.ncn_rewards();
    let jito_rewards = ncn_reward_router_account.protocol_rewards();

    msg!("Total rewards processed: {} lamports", total_rewards);
    msg!("NCN rewards: {} lamports", ncn_rewards);
    msg!("Protocol rewards: {} lamports", jito_rewards);

    msg!("Updating epoch state with final reward amounts");
    {
        let mut epoch_state_data = epoch_state.try_borrow_mut_data()?;
        let epoch_state_account = EpochState::try_from_slice_unchecked_mut(&mut epoch_state_data)?;

        epoch_state_account.update_route_ncn_rewards(ncn_rewards);
        epoch_state_account.update_route_protocol_rewards(jito_rewards);
        epoch_state_account.update_route_total_rewards(total_rewards);
    }

    Ok(())
}
