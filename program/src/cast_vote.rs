use jito_bytemuck::{AccountDeserialize, Discriminator};
use jito_jsm_core::loader::load_signer;
use jito_restaking_core::{ncn::Ncn, operator::Operator};
use ncn_program_core::{
    ballot_box::{Ballot, BallotBox, WeatherStatus},
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

pub fn process_cast_vote(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    weather_status: u8,
    epoch: u64,
) -> ProgramResult {
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

    // Operator is casting the vote, needs to be signer
    load_signer(operator_admin, false)?;

    EpochState::load(program_id, epoch_state, ncn.key, epoch, false)?;
    NcnConfig::load(program_id, ncn_config, ncn.key, false)?;
    Ncn::load(&jito_restaking_program::id(), ncn, false)?;
    Operator::load(&jito_restaking_program::id(), operator, false)?;

    BallotBox::load(program_id, ballot_box, ncn.key, epoch, true)?;
    EpochSnapshot::load(program_id, epoch_snapshot, ncn.key, epoch, false)?;
    OperatorSnapshot::load(
        program_id,
        operator_snapshot,
        operator.key,
        ncn.key,
        epoch,
        false,
    )?;
    ConsensusResult::load(program_id, consensus_result, ncn.key, epoch, true)?;

    let operator_data = operator.data.borrow();
    let operator_account = Operator::try_from_slice_unchecked(&operator_data)?;

    if *operator_admin.key != operator_account.voter {
        return Err(NCNProgramError::InvalidOperatorVoter.into());
    }

    let valid_slots_after_consensus = {
        let ncn_config_data = ncn_config.data.borrow();
        let ncn_config = NcnConfig::try_from_slice_unchecked(&ncn_config_data)?;
        ncn_config.valid_slots_after_consensus()
    };

    let mut ballot_box_data = ballot_box.data.borrow_mut();
    let ballot_box = BallotBox::try_from_slice_unchecked_mut(&mut ballot_box_data)?;

    let total_stake_weights = {
        let epoch_snapshot_data = epoch_snapshot.data.borrow();
        let epoch_snapshot = EpochSnapshot::try_from_slice_unchecked(&epoch_snapshot_data)?;

        if !epoch_snapshot.finalized() {
            return Err(NCNProgramError::EpochSnapshotNotFinalized.into());
        }

        *epoch_snapshot.stake_weights()
    };

    let operator_stake_weights = {
        let operator_snapshot_data = operator_snapshot.data.borrow();
        let operator_snapshot =
            OperatorSnapshot::try_from_slice_unchecked(&operator_snapshot_data)?;

        *operator_snapshot.stake_weights()
    };

    if operator_stake_weights.stake_weight() == 0 {
        msg!("Operator has zero stake weight, cannot vote");
        return Err(NCNProgramError::CannotVoteWithZeroStake.into());
    }

    let slot = Clock::get()?.slot;

    let ballot = Ballot::new(weather_status);

    ballot_box.cast_vote(
        operator.key,
        &ballot,
        &operator_stake_weights,
        slot,
        valid_slots_after_consensus,
    )?;

    ballot_box.tally_votes(total_stake_weights.stake_weight(), slot)?;

    // If consensus is reached, update the consensus result account
    if ballot_box.is_consensus_reached() {
        let winning_ballot_tally = ballot_box.get_winning_ballot_tally()?;
        msg!(
            "Consensus reached for epoch {} with ballot {:?}",
            epoch,
            winning_ballot_tally
        );

        // Update the consensus result account
        let mut consensus_result_data = consensus_result.try_borrow_mut_data()?;
        let consensus_result_account =
            ConsensusResult::try_from_slice_unchecked_mut(&mut consensus_result_data)?;

        consensus_result_account.record_consensus(
            winning_ballot_tally.ballot().weather_status(),
            winning_ballot_tally.stake_weights().stake_weight() as u64,
            total_stake_weights.stake_weight() as u64,
            slot,
            operator.key,
        )?;
    }

    // Update Epoch State
    {
        let mut epoch_state_data = epoch_state.try_borrow_mut_data()?;
        let epoch_state_account = EpochState::try_from_slice_unchecked_mut(&mut epoch_state_data)?;
        epoch_state_account.update_cast_vote(
            ballot_box.operators_voted(),
            ballot_box.is_consensus_reached(),
            slot,
        )?;
    }

    Ok(())
}
