use jito_bytemuck::AccountDeserialize;
use jito_jsm_core::loader::load_signer;
use jito_restaking_core::{ncn::Ncn, operator::Operator};
use ncn_program_core::{
    ballot_box::{Ballot, BallotBox},
    config::Config as NcnConfig,
    consensus_result::ConsensusResult,
    epoch_snapshot::{EpochSnapshot, OperatorSnapshot},
    epoch_state::EpochState,
    error::NCNProgramError,
};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    pubkey::Pubkey,
    sysvar::Sysvar,
};

/// Allows an operator to cast a vote on weather status.
///
/// ### Parameters:
/// - `weather_status`: Status code for the vote (0=Sunny, 1=Cloudy, 2=Rainy)
/// - `epoch`: The target epoch
///
/// ### Accounts:
/// 1. `[writable]` epoch_state: The epoch state account for the target epoch
/// 2. `[]` config: NCN configuration account (named `ncn_config` in code)
/// 3. `[writable]` ballot_box: The ballot box for recording votes
/// 4. `[]` ncn: The NCN account
/// 5. `[]` epoch_snapshot: Epoch snapshot containing stake weights
/// 6. `[]` operator_snapshot: Operator snapshot containing operator stake
/// 7. `[]` operator: The operator account casting the vote
/// 8. `[signer]` operator_admin: The account authorized to vote on behalf of the operator (referred to as `operator_voter` in some docs)
/// 9. `[writable]` consensus_result: Account for storing the consensus result
pub fn process_cast_vote(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    weather_status: u8,
    epoch: u64,
) -> ProgramResult {
    msg!(
        "Processing cast vote for epoch: {}, weather status: {}",
        epoch,
        weather_status
    );

    let account_info_iter = &mut accounts.iter();
    let epoch_state = next_account_info(account_info_iter)?;
    let ncn_config = next_account_info(account_info_iter)?;
    let ballot_box = next_account_info(account_info_iter)?;
    let ncn = next_account_info(account_info_iter)?;
    let epoch_snapshot = next_account_info(account_info_iter)?;
    let operator_snapshot = next_account_info(account_info_iter)?;
    let operator = next_account_info(account_info_iter)?;
    let operator_admin = next_account_info(account_info_iter)?;
    let consensus_result = next_account_info(account_info_iter)?;

    msg!("Verifying operator admin is the signer");
    // Operator is casting the vote, needs to be signer
    load_signer(operator_admin, false)?;

    msg!("Loading epoch state for NCN: {}, epoch: {}", ncn.key, epoch);
    EpochState::load(program_id, epoch_state, ncn.key, epoch, false)?;

    msg!("Loading NCN config for NCN: {}", ncn.key);
    NcnConfig::load(program_id, ncn_config, ncn.key, false)?;

    msg!("Loading NCN account: {}", ncn.key);
    Ncn::load(&jito_restaking_program::id(), ncn, false)?;

    msg!("Loading operator account: {}", operator.key);
    Operator::load(&jito_restaking_program::id(), operator, false)?;

    msg!("Loading ballot box for NCN: {}, epoch: {}", ncn.key, epoch);
    BallotBox::load(program_id, ballot_box, ncn.key, epoch, true)?;

    msg!(
        "Loading epoch snapshot for NCN: {}, epoch: {}",
        ncn.key,
        epoch
    );
    EpochSnapshot::load(program_id, epoch_snapshot, ncn.key, epoch, false)?;

    msg!(
        "Loading operator snapshot for operator: {}, NCN: {}, epoch: {}",
        operator.key,
        ncn.key,
        epoch
    );
    OperatorSnapshot::load(
        program_id,
        operator_snapshot,
        operator.key,
        ncn.key,
        epoch,
        false,
    )?;

    msg!(
        "Loading consensus result for NCN: {}, epoch: {}",
        ncn.key,
        epoch
    );
    ConsensusResult::load(program_id, consensus_result, ncn.key, epoch, true)?;

    msg!("Verifying operator admin is the designated voter");
    let operator_data = operator.data.borrow();
    let operator_account = Operator::try_from_slice_unchecked(&operator_data)?;

    if *operator_admin.key != operator_account.voter {
        msg!(
            "Error: Invalid operator voter. Expected: {}, got: {}",
            operator_account.voter,
            operator_admin.key
        );
        return Err(NCNProgramError::InvalidOperatorVoter.into());
    }

    let valid_slots_after_consensus = {
        let ncn_config_data = ncn_config.data.borrow();
        let ncn_config = NcnConfig::try_from_slice_unchecked(&ncn_config_data)?;
        ncn_config.valid_slots_after_consensus()
    };
    msg!(
        "Valid slots after consensus: {}",
        valid_slots_after_consensus
    );

    let mut ballot_box_data = ballot_box.data.borrow_mut();
    let ballot_box = BallotBox::try_from_slice_unchecked_mut(&mut ballot_box_data)?;

    msg!("Getting total stake weights from epoch snapshot");
    let total_stake_weights = {
        let epoch_snapshot_data = epoch_snapshot.data.borrow();
        let epoch_snapshot = EpochSnapshot::try_from_slice_unchecked(&epoch_snapshot_data)?;

        if !epoch_snapshot.finalized() {
            msg!("Error: Epoch snapshot not finalized for epoch: {}", epoch);
            return Err(NCNProgramError::EpochSnapshotNotFinalized.into());
        }

        *epoch_snapshot.stake_weights()
    };
    msg!("Total stake weight: {}", total_stake_weights.stake_weight());

    let operator_stake_weights = {
        let operator_snapshot_data = operator_snapshot.data.borrow();
        let operator_snapshot =
            OperatorSnapshot::try_from_slice_unchecked(&operator_snapshot_data)?;

        *operator_snapshot.stake_weights()
    };
    msg!(
        "Operator stake weight: {}",
        operator_stake_weights.stake_weight()
    );

    if operator_stake_weights.stake_weight() == 0 {
        msg!("Error: Operator has zero stake weight, cannot vote");
        return Err(NCNProgramError::CannotVoteWithZeroStake.into());
    }

    let slot = Clock::get()?.slot;
    msg!("Current slot: {}", slot);

    let ballot = Ballot::new(weather_status);

    ballot_box.cast_vote(
        operator.key,
        &ballot,
        &operator_stake_weights,
        slot,
        valid_slots_after_consensus,
    )?;

    msg!(
        "Tallying votes with total stake weight: {}, current slot: {}",
        total_stake_weights.stake_weight(),
        slot
    );
    ballot_box.tally_votes(total_stake_weights.stake_weight(), slot)?;

    // If consensus is reached, update the consensus result account
    if ballot_box.is_consensus_reached() {
        msg!("Consensus has been reached for epoch: {}", epoch);
        let winning_ballot_tally = ballot_box.get_winning_ballot_tally()?;
        msg!(
            "Consensus reached for epoch {} with ballot weather status: {}, stake weight: {}",
            epoch,
            winning_ballot_tally.ballot().weather_status(),
            winning_ballot_tally.stake_weights().stake_weight()
        );

        // Update the consensus result account
        msg!("Updating consensus result account");
        let mut consensus_result_data = consensus_result.try_borrow_mut_data()?;
        let consensus_result_account =
            ConsensusResult::try_from_slice_unchecked_mut(&mut consensus_result_data)?;

        consensus_result_account.record_consensus(
            winning_ballot_tally.ballot().weather_status(),
            winning_ballot_tally.stake_weights().stake_weight() as u64,
            total_stake_weights.stake_weight() as u64,
            slot,
        )?;
    } else {
        msg!("Consensus not yet reached for epoch: {}", epoch);
    }

    // Update Epoch State
    msg!("Updating epoch state account");
    {
        let mut epoch_state_data = epoch_state.try_borrow_mut_data()?;
        let epoch_state_account = EpochState::try_from_slice_unchecked_mut(&mut epoch_state_data)?;
        epoch_state_account.update_cast_vote(
            ballot_box.operators_voted(),
            ballot_box.is_consensus_reached(),
            slot,
        )?;
    }

    msg!(
        "Cast vote completed successfully for operator: {}, epoch: {}, weather status: {}",
        operator.key,
        epoch,
        weather_status
    );

    Ok(())
}
