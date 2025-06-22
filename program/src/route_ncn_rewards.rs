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
    msg!("=== NCN Reward Routing Process Started ===");
    msg!("Starting NCN reward routing for epoch: {}", epoch);
    msg!("Program ID: {}", program_id);
    msg!(
        "Max iterations for operator vault routing: {}",
        max_iterations
    );
    msg!("Number of accounts provided: {}", accounts.len());

    let [epoch_state, config, ncn, epoch_snapshot, ballot_box, ncn_reward_router, ncn_reward_receiver] =
        accounts
    else {
        msg!("ERROR: Incorrect number of accounts provided");
        msg!("Expected 7 accounts, got: {}", accounts.len());
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    msg!("=== Account Validation Phase ===");
    msg!("Validating account keys and loading data...");

    msg!("Loading EpochState account: {}", epoch_state.key);
    EpochState::load(program_id, epoch_state, ncn.key, epoch, true)?;
    msg!("✓ EpochState loaded successfully for epoch {}", epoch);

    msg!("Loading NCN Config account: {}", config.key);
    NcnConfig::load(program_id, config, ncn.key, false)?;
    msg!("✓ NCN Config loaded successfully");

    msg!("Loading NCN account: {}", ncn.key);
    Ncn::load(&jito_restaking_program::id(), ncn, false)?;
    msg!("✓ NCN account loaded successfully");

    msg!("Loading EpochSnapshot account: {}", epoch_snapshot.key);
    EpochSnapshot::load(program_id, epoch_snapshot, ncn.key, epoch, false)?;
    msg!("✓ EpochSnapshot loaded successfully for epoch {}", epoch);

    msg!("Loading NCNRewardRouter account: {}", ncn_reward_router.key);
    NCNRewardRouter::load(program_id, ncn_reward_router, ncn.key, epoch, true)?;
    msg!("✓ NCNRewardRouter loaded successfully for epoch {}", epoch);

    msg!("Loading BallotBox account: {}", ballot_box.key);
    BallotBox::load(program_id, ballot_box, ncn.key, epoch, false)?;
    msg!("✓ BallotBox loaded successfully for epoch {}", epoch);

    msg!(
        "Loading NCNRewardReceiver account: {}",
        ncn_reward_receiver.key
    );
    NCNRewardReceiver::load(program_id, ncn_reward_receiver, ncn.key, epoch, true)?;
    msg!(
        "✓ NCNRewardReceiver loaded successfully for epoch {}",
        epoch
    );

    msg!("=== Data Processing Phase ===");
    msg!("Deserializing account data for processing...");

    let epoch_snapshot_data = epoch_snapshot.try_borrow_data()?;
    let epoch_snapshot_account = EpochSnapshot::try_from_slice_unchecked(&epoch_snapshot_data)?;
    msg!("✓ EpochSnapshot data deserialized successfully");

    let ballot_box_data = ballot_box.try_borrow_data()?;
    let ballot_box_account = BallotBox::try_from_slice_unchecked(&ballot_box_data)?;
    msg!("✓ BallotBox data deserialized successfully");

    msg!("=== Voting Status Check ===");
    msg!("Checking if voting period has ended...");
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
        msg!("❌ VOTING STILL ONGOING - Cannot route rewards yet");
        msg!(
            "Current slot: {}, Valid slots after consensus: {}",
            current_slot,
            valid_slots_after_consensus
        );
        msg!("Voting period must end before rewards can be routed");
        return Err(NCNProgramError::VotingIsNotOver.into());
    }
    msg!("✓ Voting period has ended, proceeding with reward routing");

    msg!("=== Reward Balance Check ===");
    let ncn_reward_receiver_balance = **ncn_reward_receiver.try_borrow_lamports()?;
    msg!(
        "NCN reward receiver balance: {} lamports",
        ncn_reward_receiver_balance
    );
    msg!("NCN reward receiver address: {}", ncn_reward_receiver.key);

    msg!("=== Reward Router Processing ===");
    let mut ncn_reward_router_data = ncn_reward_router.try_borrow_mut_data()?;
    let ncn_reward_router_account =
        NCNRewardRouter::try_from_slice_unchecked_mut(&mut ncn_reward_router_data)?;

    let rent_cost = Rent::get()?.minimum_balance(0);
    msg!("Rent cost calculated: {} lamports", rent_cost);

    if !ncn_reward_router_account.still_routing() {
        msg!("=== Processing Incoming Rewards ===");
        msg!("NCN reward router is not still routing, processing incoming rewards and reward pool");

        msg!("Routing incoming rewards...");
        ncn_reward_router_account.route_incoming_rewards(rent_cost, ncn_reward_receiver_balance)?;
        msg!("✓ Incoming rewards routed successfully");

        msg!("=== Processing Reward Pool ===");
        let epoch_fees = epoch_snapshot_account.fees();
        msg!("Epoch fees from snapshot: {:?}", epoch_fees);
        msg!("Routing reward pool with epoch fees...");
        ncn_reward_router_account.route_reward_pool(epoch_fees)?;
        msg!("✓ Reward pool routed successfully");
        msg!("Total epoch fees processed: {:?}", epoch_fees);
    } else {
        msg!("=== Skipping Incoming Rewards ===");
        msg!(
            "NCN reward router is still routing, skipping incoming rewards and reward pool routing"
        );
        msg!("This indicates rewards are already being processed from a previous call");
    }

    msg!("=== Operator Vault Rewards Routing ===");
    msg!(
        "Starting operator vault rewards routing with max iterations: {}",
        max_iterations
    );
    ncn_reward_router_account.route_operator_vault_rewards(ballot_box_account, max_iterations)?;
    msg!("✓ Operator vault rewards routing completed successfully");

    msg!("=== Final Reward Summary ===");
    let total_rewards = ncn_reward_router_account.total_rewards();
    let ncn_rewards = ncn_reward_router_account.ncn_rewards();
    let jito_rewards = ncn_reward_router_account.jito_dao_rewards();

    msg!("Total rewards processed: {} lamports", total_rewards);
    msg!("NCN rewards: {} lamports", ncn_rewards);
    msg!("Jito DAO rewards: {} lamports", jito_rewards);

    msg!("=== Updating Epoch State ===");
    msg!("Updating epoch state with final reward amounts...");
    {
        let mut epoch_state_data = epoch_state.try_borrow_mut_data()?;
        let epoch_state_account = EpochState::try_from_slice_unchecked_mut(&mut epoch_state_data)?;

        msg!(
            "Previous NCN rewards in epoch state: {} lamports",
            epoch_state_account.ncn_distribution_progress().total()
        );
        msg!(
            "Previous Jito DAO rewards in epoch state: {} lamports",
            epoch_state_account.jito_dao_distribution_progress().total()
        );
        msg!(
            "Previous total rewards in epoch state: {} lamports",
            epoch_state_account.total_distribution_progress().total()
        );

        epoch_state_account.update_route_ncn_rewards(ncn_rewards);
        epoch_state_account.update_route_jito_dao_rewards(jito_rewards);
        epoch_state_account.update_route_total_rewards(total_rewards);

        msg!("✓ Epoch state updated successfully");
        msg!(
            "New NCN rewards in epoch state: {} lamports",
            epoch_state_account.ncn_distribution_progress().total()
        );
        msg!(
            "New Jito DAO rewards in epoch state: {} lamports",
            epoch_state_account.jito_dao_distribution_progress().total()
        );
        msg!(
            "New total rewards in epoch state: {} lamports",
            epoch_state_account.total_distribution_progress().total()
        );
    }

    msg!("=== NCN Reward Routing Process Completed ===");
    msg!(
        "NCN reward routing completed successfully for epoch: {}",
        epoch
    );
    msg!("All rewards have been processed and distributed");
    Ok(())
}
