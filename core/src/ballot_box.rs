// Ballot Box Module
//
// This module implements the core voting and consensus mechanism for the NCN program.
// It allows operators to cast votes on proposed states (represented as 'weather statuses'),
// tallies those votes weighted by stake, and determines when consensus has been reached.
//
// Key components:
// - WeatherStatus: Represents different possible states that validators vote on
// - Ballot: A single vote for a particular weather status
// - BallotTally: Tracks total votes and stake weight for a specific ballot type
// - OperatorVote: Records an individual operator's vote
// - BallotBox: The main structure that manages the entire voting process
//
// The consensus mechanism requires a 2/3 majority of stake weight to agree on
// a particular ballot before it is considered the winning consensus state.
// The system includes features for tie-breaking and detecting stalled votes.

use core::fmt;
use std::mem::size_of;

use bytemuck::{Pod, Zeroable};
use jito_bytemuck::{
    types::{PodBool, PodU16, PodU64},
    AccountDeserialize, Discriminator,
};
use shank::{ShankAccount, ShankType};
use solana_program::{account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey};
use spl_math::precise_number::PreciseNumber;

use crate::{
    constants::{precise_consensus, DEFAULT_CONSENSUS_REACHED_SLOT, MAX_OPERATORS},
    discriminators::Discriminators,
    error::NCNProgramError,
    loaders::check_load,
    stake_weight::StakeWeights,
};

/// Enum representing weather status
#[derive(Debug, Default, Clone, Copy, Zeroable, PartialEq, Eq)]
#[repr(C)]
pub enum WeatherStatus {
    /// Clear sunny weather
    #[default]
    Sunny = 0,
    /// Cloudy weather conditions
    Cloudy = 1,
    /// Rainy weather conditions
    Rainy = 2,
}

impl WeatherStatus {
    /// Converts a u8 value to a weather status string
    /// Returns None if the value is invalid
    pub fn from_u8(value: u8) -> Option<&'static str> {
        match value {
            0 => Some("Sunny"),
            1 => Some("Cloudy"),
            2 => Some("Rainy"),
            _ => None,
        }
    }
}

impl fmt::Display for WeatherStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status_str = match self {
            WeatherStatus::Sunny => "Sunny",
            WeatherStatus::Cloudy => "Cloudy",
            WeatherStatus::Rainy => "Rainy",
        };
        write!(f, "{}", status_str)
    }
}

/// Represents a ballot with a weather status
#[derive(Debug, Clone, Copy, Zeroable, ShankType, Pod)]
#[repr(C)]
pub struct Ballot {
    /// The weather status value
    weather_status: u8,
    /// Whether the ballot is valid
    is_valid: PodBool,
}

impl PartialEq for Ballot {
    fn eq(&self, other: &Self) -> bool {
        if !self.is_valid() || !other.is_valid() {
            return false;
        }
        self.weather_status == other.weather_status
    }
}

impl Eq for Ballot {}

impl Default for Ballot {
    fn default() -> Self {
        Self {
            weather_status: WeatherStatus::default() as u8,
            is_valid: PodBool::from(false),
        }
    }
}

impl std::fmt::Display for Ballot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            WeatherStatus::from_u8(self.weather_status).unwrap_or("Invalid")
        )
    }
}

impl Ballot {
    pub fn new(weather_status: u8) -> Self {
        let mut ballot = Self {
            weather_status,
            is_valid: PodBool::from(false),
        };

        // Only valid if it matches a WeatherStatus variant
        if weather_status <= WeatherStatus::Rainy as u8 {
            ballot.is_valid = PodBool::from(true);
        }

        ballot
    }

    pub const fn weather_status(&self) -> u8 {
        self.weather_status
    }

    pub fn status(&self) -> Option<&'static str> {
        WeatherStatus::from_u8(self.weather_status)
    }

    pub fn is_valid(&self) -> bool {
        self.is_valid.into()
    }
}

/// Represents a tally of votes for a specific ballot
#[derive(Debug, Clone, Copy, Zeroable, ShankType, Pod)]
#[repr(C)]
pub struct BallotTally {
    /// Index of the tally within the ballot_tallies
    index: PodU16,
    /// The ballot being tallied
    ballot: Ballot,
    /// Breakdown of all of the stake weights that contribute to the vote
    stake_weights: StakeWeights,
    /// The number of votes for this ballot
    tally: PodU64,
}

impl Default for BallotTally {
    fn default() -> Self {
        Self {
            index: PodU16::from(u16::MAX),
            ballot: Ballot::default(),
            stake_weights: StakeWeights::default(),
            tally: PodU64::from(0),
        }
    }
}

impl BallotTally {
    pub fn new(index: u16, ballot: &Ballot, stake_weights: &StakeWeights) -> Self {
        Self {
            index: PodU16::from(index),
            ballot: *ballot,
            stake_weights: *stake_weights,
            tally: PodU64::from(1),
        }
    }

    pub const fn ballot(&self) -> &Ballot {
        &self.ballot
    }

    pub const fn stake_weights(&self) -> &StakeWeights {
        &self.stake_weights
    }

    pub fn tally(&self) -> u64 {
        self.tally.into()
    }

    pub fn index(&self) -> u16 {
        self.index.into()
    }

    pub fn is_valid(&self) -> bool {
        self.ballot.is_valid()
    }

    pub fn increment_tally(&mut self, stake_weights: &StakeWeights) -> Result<(), NCNProgramError> {
        self.stake_weights.increment(stake_weights)?;
        self.tally = PodU64::from(
            self.tally()
                .checked_add(1)
                .ok_or(NCNProgramError::ArithmeticOverflow)?,
        );

        Ok(())
    }

    pub fn decrement_tally(&mut self, stake_weights: &StakeWeights) -> Result<(), NCNProgramError> {
        self.stake_weights.decrement(stake_weights)?;
        self.tally = PodU64::from(
            self.tally()
                .checked_sub(1)
                .ok_or(NCNProgramError::ArithmeticOverflow)?,
        );

        Ok(())
    }
}

/// Represents a vote cast by an operator
#[derive(Debug, Clone, Copy, Zeroable, ShankType, Pod)]
#[repr(C)]
pub struct OperatorVote {
    /// The operator that cast the vote
    operator: Pubkey,
    /// The slot when the operator voted
    slot_voted: PodU64,
    /// The stake weights of the operator
    stake_weights: StakeWeights,
    /// The index of the ballot in the ballot_tallies
    ballot_index: PodU16,
}

impl Default for OperatorVote {
    fn default() -> Self {
        Self {
            operator: Pubkey::default(),
            slot_voted: PodU64::from(0),
            stake_weights: StakeWeights::default(),
            ballot_index: PodU16::from(u16::MAX),
        }
    }
}

impl OperatorVote {
    pub fn new(
        ballot_index: usize,
        operator: &Pubkey,
        current_slot: u64,
        stake_weights: &StakeWeights,
    ) -> Self {
        Self {
            operator: *operator,
            ballot_index: PodU16::from(ballot_index as u16),
            slot_voted: PodU64::from(current_slot),
            stake_weights: *stake_weights,
        }
    }

    pub const fn operator(&self) -> &Pubkey {
        &self.operator
    }

    pub fn slot_voted(&self) -> u64 {
        self.slot_voted.into()
    }

    pub const fn stake_weights(&self) -> &StakeWeights {
        &self.stake_weights
    }

    pub fn ballot_index(&self) -> u16 {
        self.ballot_index.into()
    }

    pub fn is_empty(&self) -> bool {
        self.ballot_index() == u16::MAX
    }
}

/// PDA'd ["ballot_box", NCN, NCN_EPOCH_SLOT]
/// Represents a ballot box for collecting and tallying votes
#[derive(Debug, Clone, Copy, Zeroable, Pod, AccountDeserialize, ShankAccount)]
#[repr(C)]
pub struct BallotBox {
    /// The NCN account this ballot box is for
    ncn: Pubkey,
    /// The epoch this ballot box is for
    epoch: PodU64,
    /// Bump seed for the PDA
    bump: u8,
    /// Slot when this ballot box was created
    slot_created: PodU64,
    /// Slot when consensus was reached
    slot_consensus_reached: PodU64,
    /// Number of operators that have voted
    operators_voted: PodU64,
    /// Number of unique ballots
    unique_ballots: PodU64,
    /// The ballot that got at least 66% of votes
    winning_ballot: Ballot,
    /// Operator votes
    operator_votes: [OperatorVote; 256],
    /// Mapping of ballots votes to stake weight
    ballot_tallies: [BallotTally; 256],
}

impl Discriminator for BallotBox {
    const DISCRIMINATOR: u8 = Discriminators::BallotBox as u8;
}

impl BallotBox {
    const BALLOT_BOX_SEED: &'static [u8] = b"ballot_box";
    pub const SIZE: usize = 8 + size_of::<Self>();

    pub fn new(ncn: &Pubkey, epoch: u64, bump: u8, current_slot: u64) -> Self {
        Self {
            ncn: *ncn,
            epoch: PodU64::from(epoch),
            bump,
            slot_created: PodU64::from(current_slot),
            slot_consensus_reached: PodU64::from(DEFAULT_CONSENSUS_REACHED_SLOT),
            operators_voted: PodU64::from(0),
            unique_ballots: PodU64::from(0),
            winning_ballot: Ballot::default(),
            operator_votes: [OperatorVote::default(); MAX_OPERATORS],
            ballot_tallies: [BallotTally::default(); MAX_OPERATORS],
        }
    }

    pub fn initialize(&mut self, ncn: &Pubkey, epoch: u64, bump: u8, current_slot: u64) {
        // Avoids overflowing stack
        self.ncn = *ncn;
        self.epoch = PodU64::from(epoch);
        self.bump = bump;
        self.slot_created = PodU64::from(current_slot);
        self.slot_consensus_reached = PodU64::from(DEFAULT_CONSENSUS_REACHED_SLOT);
        self.operators_voted = PodU64::from(0);
        self.unique_ballots = PodU64::from(0);
        self.winning_ballot = Ballot::default();
        self.operator_votes = [OperatorVote::default(); MAX_OPERATORS];
        self.ballot_tallies = [BallotTally::default(); MAX_OPERATORS];
    }

    pub fn seeds(ncn: &Pubkey, epoch: u64) -> Vec<Vec<u8>> {
        Vec::from_iter(
            [
                Self::BALLOT_BOX_SEED.to_vec(),
                ncn.to_bytes().to_vec(),
                epoch.to_le_bytes().to_vec(),
            ]
            .iter()
            .cloned(),
        )
    }

    pub fn find_program_address(
        program_id: &Pubkey,
        ncn: &Pubkey,
        epoch: u64,
    ) -> (Pubkey, u8, Vec<Vec<u8>>) {
        let seeds = Self::seeds(ncn, epoch);
        let seeds_iter: Vec<_> = seeds.iter().map(|s| s.as_slice()).collect();
        let (pda, bump) = Pubkey::find_program_address(&seeds_iter, program_id);
        (pda, bump, seeds)
    }

    pub fn load(
        program_id: &Pubkey,
        account: &AccountInfo,
        ncn: &Pubkey,
        epoch: u64,
        expect_writable: bool,
    ) -> Result<(), ProgramError> {
        let expected_pda = Self::find_program_address(program_id, ncn, epoch).0;
        check_load(
            program_id,
            account,
            &expected_pda,
            Some(Self::DISCRIMINATOR),
            expect_writable,
        )
    }

    pub fn load_to_close(
        program_id: &Pubkey,
        account_to_close: &AccountInfo,
        ncn: &Pubkey,
        epoch: u64,
    ) -> Result<(), ProgramError> {
        Self::load(program_id, account_to_close, ncn, epoch, true)
    }

    pub fn epoch(&self) -> u64 {
        self.epoch.into()
    }

    pub fn slot_consensus_reached(&self) -> u64 {
        self.slot_consensus_reached.into()
    }

    pub fn unique_ballots(&self) -> u64 {
        self.unique_ballots.into()
    }

    pub fn operators_voted(&self) -> u64 {
        self.operators_voted.into()
    }

    pub fn has_ballot(&self, ballot: &Ballot) -> bool {
        self.ballot_tallies.iter().any(|t| t.ballot.eq(ballot))
    }

    pub const fn ballot_tallies(&self) -> &[BallotTally; MAX_OPERATORS] {
        &self.ballot_tallies
    }

    pub fn is_consensus_reached(&self) -> bool {
        self.slot_consensus_reached() != DEFAULT_CONSENSUS_REACHED_SLOT
            || self.winning_ballot.is_valid()
    }

    pub fn tie_breaker_set(&self) -> bool {
        self.slot_consensus_reached() == DEFAULT_CONSENSUS_REACHED_SLOT
            && self.winning_ballot.is_valid()
    }

    pub fn get_winning_ballot(&self) -> Result<&Ballot, NCNProgramError> {
        if !self.winning_ballot.is_valid() {
            Err(NCNProgramError::ConsensusNotReached)
        } else {
            Ok(&self.winning_ballot)
        }
    }

    pub fn get_winning_ballot_tally(&self) -> Result<&BallotTally, NCNProgramError> {
        if !self.winning_ballot.is_valid() {
            Err(NCNProgramError::ConsensusNotReached)
        } else {
            let winning_ballot_tally = self
                .ballot_tallies
                .iter()
                .find(|t| t.ballot.eq(&self.winning_ballot))
                .ok_or(NCNProgramError::BallotTallyNotFoundFull)?;

            Ok(winning_ballot_tally)
        }
    }

    pub fn has_winning_ballot(&self) -> bool {
        self.winning_ballot.is_valid()
    }

    pub const fn operator_votes(&self) -> &[OperatorVote; MAX_OPERATORS] {
        &self.operator_votes
    }

    pub fn set_winning_ballot(&mut self, ballot: &Ballot) {
        self.winning_ballot = *ballot;
    }

    fn increment_or_create_ballot_tally(
        &mut self,
        ballot: &Ballot,
        stake_weights: &StakeWeights,
    ) -> Result<usize, NCNProgramError> {
        let result = self
            .ballot_tallies
            .iter()
            .enumerate()
            .find(|(_, t)| t.is_valid() && t.ballot.eq(ballot));

        if let Some((tally_index, _)) = result {
            self.ballot_tallies[tally_index].increment_tally(stake_weights)?;
            return Ok(tally_index);
        }

        for (tally_index, tally) in self.ballot_tallies.iter_mut().enumerate() {
            if !tally.is_valid() {
                *tally = BallotTally::new(tally_index as u16, ballot, stake_weights);

                self.unique_ballots = PodU64::from(
                    self.unique_ballots()
                        .checked_add(1)
                        .ok_or(NCNProgramError::ArithmeticOverflow)?,
                );

                return Ok(tally_index);
            }
        }

        Err(NCNProgramError::BallotTallyFull)
    }

    /// Casts a vote for a ballot from an operator
    /// Returns error if:
    /// - Operator has already voted
    /// - Voting is not valid
    /// - Bad ballot
    /// - Consensus already reached
    /// - Operator votes are full
    pub fn cast_vote(
        &mut self,
        operator: &Pubkey,
        ballot: &Ballot,
        stake_weights: &StakeWeights,
        current_slot: u64,
        valid_slots_after_consensus: u64,
    ) -> Result<(), NCNProgramError> {
        if !self.is_voting_valid(current_slot, valid_slots_after_consensus)? {
            return Err(NCNProgramError::VotingNotValid);
        }

        if !ballot.is_valid() {
            return Err(NCNProgramError::BadBallot);
        }

        // Check if operator has already voted
        for vote in self.operator_votes.iter() {
            if vote.operator().eq(operator) {
                return Err(NCNProgramError::OperatorAlreadyVoted);
            }
        }

        let ballot_index = self.increment_or_create_ballot_tally(ballot, stake_weights)?;

        // Find empty slot for new vote
        for vote in self.operator_votes.iter_mut() {
            if vote.is_empty() {
                *vote = OperatorVote::new(ballot_index, operator, current_slot, stake_weights);
                self.operators_voted = PodU64::from(
                    self.operators_voted()
                        .checked_add(1)
                        .ok_or(NCNProgramError::ArithmeticOverflow)?,
                );
                return Ok(());
            }
        }

        Err(NCNProgramError::OperatorVotesFull)
    }

    /// Tallies all votes and determines if consensus has been reached
    /// Updates the winning ballot if consensus threshold is met
    pub fn tally_votes(
        &mut self,
        total_stake_weight: u128,
        current_slot: u64,
    ) -> Result<(), NCNProgramError> {
        if self.slot_consensus_reached() != DEFAULT_CONSENSUS_REACHED_SLOT {
            return Ok(());
        }

        // Find ballot with maximum stake weight
        let max_tally = self
            .ballot_tallies
            .iter()
            .max_by_key(|t| t.stake_weights().stake_weight())
            .ok_or(NCNProgramError::NoValidBallots)?;

        let ballot_stake_weight = max_tally.stake_weights().stake_weight();

        // Prevent division by zero
        if total_stake_weight == 0 {
            return Err(NCNProgramError::DenominatorIsZero);
        }

        let precise_ballot_stake_weight = PreciseNumber::new(ballot_stake_weight)
            .ok_or(NCNProgramError::NewPreciseNumberError)?;
        let precise_total_stake_weight =
            PreciseNumber::new(total_stake_weight).ok_or(NCNProgramError::NewPreciseNumberError)?;

        let ballot_percentage_of_total = precise_ballot_stake_weight
            .checked_div(&precise_total_stake_weight)
            .ok_or(NCNProgramError::DenominatorIsZero)?;

        let target_precise_percentage = precise_consensus()?;

        let consensus_reached =
            ballot_percentage_of_total.greater_than_or_equal(&target_precise_percentage);

        if consensus_reached && !self.winning_ballot.is_valid() {
            self.slot_consensus_reached = PodU64::from(current_slot);
            let winning_ballot = *max_tally.ballot();
            self.set_winning_ballot(&winning_ballot);
        }

        Ok(())
    }

    /// Sets a tie breaker ballot when voting is stalled
    /// Only allows setting a ballot that was previously voted on
    pub fn set_tie_breaker_ballot(
        &mut self,
        weather_status: u8,
        current_epoch: u64,
        epochs_before_stall: u64,
    ) -> Result<(), NCNProgramError> {
        // Check that consensus has not been reached
        if self.is_consensus_reached() {
            return Err(NCNProgramError::ConsensusAlreadyReached);
        }

        // Check if voting is stalled and setting the tie breaker is eligible
        let stall_epoch = self
            .epoch()
            .checked_add(epochs_before_stall)
            .ok_or(NCNProgramError::ArithmeticOverflow)?;

        if current_epoch < stall_epoch {
            return Err(NCNProgramError::VotingNotFinalized);
        }

        // Validate weather status
        if weather_status > WeatherStatus::Rainy as u8 {
            return Err(NCNProgramError::BadBallot);
        }

        let finalized_ballot = Ballot::new(weather_status);

        // Check that the ballot is one of the existing options
        if !self.has_ballot(&finalized_ballot) {
            return Err(NCNProgramError::TieBreakerNotInPriorVotes);
        }

        self.set_winning_ballot(&finalized_ballot);
        Ok(())
    }

    /// Determines if an operator can still cast their vote.
    /// Returns true when:
    /// Consensus is not reached OR the voting window is still valid, assuming set_tie_breaker was not invoked
    pub fn is_voting_valid(
        &self,
        current_slot: u64,
        valid_slots_after_consensus: u64,
    ) -> Result<bool, NCNProgramError> {
        if self.tie_breaker_set() {
            return Ok(false);
        }

        if self.is_consensus_reached() {
            let vote_window_valid = current_slot
                <= self
                    .slot_consensus_reached()
                    .checked_add(valid_slots_after_consensus)
                    .ok_or(NCNProgramError::ArithmeticOverflow)?;

            return Ok(vote_window_valid);
        }

        Ok(true)
    }
}

#[rustfmt::skip]
impl fmt::Display for BallotBox {
   fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
       writeln!(f, "\n\n----------- Ballot Box -------------")?;
       writeln!(f, "  NCN:                          {}", self.ncn)?;
       writeln!(f, "  Epoch:                        {}", self.epoch())?;
       writeln!(f, "  Bump:                         {}", self.bump)?;
       writeln!(f, "  Slot Consensus Reached:       {}", self.slot_consensus_reached())?;
       writeln!(f, "  Operators Voted:              {}", self.operators_voted())?;
       writeln!(f, "  Unique Ballots:               {}", self.unique_ballots())?;
       writeln!(f, "  IS Consensus Reached:         {}", self.is_consensus_reached())?;
       if self.is_consensus_reached() {
           writeln!(f, "  Tie Breaker Set:              {}", self.tie_breaker_set())?;
           if let Ok(winning_ballot) = self.get_winning_ballot() {
               writeln!(f, "  Winning Ballot:               {}", winning_ballot)?;
           }
       }

       writeln!(f, "\nOperator Votes:")?;
       for vote in self.operator_votes().iter() {
           if !vote.is_empty() {
               writeln!(f, "  Operator:                     {}", vote.operator())?;
               writeln!(f, "    Slot Voted:                 {}", vote.slot_voted())?;
               writeln!(f, "    Ballot Index:               {}", vote.ballot_index())?;
               writeln!(f, "    Stake Weights:")?;
           }
       }

       writeln!(f, "\nBallot Tallies:")?;
       for tally in self.ballot_tallies().iter() {
           if tally.is_valid() {
               writeln!(f, "  Index {}:", tally.index())?;
               writeln!(f, "    Ballot:                     {}", tally.ballot())?;
               writeln!(f, "    Tally:                      {}", tally.tally())?;
               writeln!(f, "    Stake Weights:              {}", tally.stake_weights().stake_weight())?;
           }
       }

       writeln!(f, "\n")?;
       Ok(())
   }
}

#[cfg(test)]
mod tests {
    use solana_program::msg;

    use crate::utils::assert_ncn_program_error;

    use super::*;

    #[test]
    fn test_len() {
        use std::mem::size_of;

        let expected_total = size_of::<Pubkey>() // ncn
            + size_of::<PodU64>() // epoch
            + 1 // bump
            + size_of::<PodU64>() // slot_created
            + size_of::<PodU64>() // slot_consensus_reached
            + size_of::<PodU64>() // operators_voted
            + size_of::<PodU64>() // unique_ballots
            + size_of::<Ballot>() // winning_ballot
            + size_of::<OperatorVote>() * MAX_OPERATORS // operator_votes
            + size_of::<BallotTally>() * MAX_OPERATORS; // ballot_tallies

        assert_eq!(size_of::<BallotBox>(), expected_total);

        let ballot_box = BallotBox::new(&Pubkey::default(), 0, 0, 0);
        assert_eq!(ballot_box.operator_votes.len(), MAX_OPERATORS);
        assert_eq!(ballot_box.ballot_tallies.len(), MAX_OPERATORS);
    }

    #[test]
    fn test_cast_vote() {
        let ncn = Pubkey::new_unique();
        let operator = Pubkey::new_unique();
        let current_slot = 100;
        let epoch = 1;
        let stake_weights = StakeWeights::new(1000);
        let valid_slots_after_consensus = 10;
        let mut ballot_box = BallotBox::new(&ncn, epoch, 0, current_slot);
        let ballot = Ballot::new(WeatherStatus::Sunny as u8);

        // Test initial cast vote
        ballot_box
            .cast_vote(
                &operator,
                &ballot,
                &stake_weights,
                current_slot,
                valid_slots_after_consensus,
            )
            .unwrap();

        // Verify vote was recorded correctly
        {
            let operator_vote = ballot_box
                .operator_votes
                .iter()
                .find(|v| v.operator().eq(&operator))
                .unwrap();
            assert_eq!(
                operator_vote.stake_weights().stake_weight(),
                stake_weights.stake_weight()
            );
            assert_eq!(operator_vote.slot_voted(), current_slot);

            // Verify ballot tally
            let tally = ballot_box
                .ballot_tallies
                .iter()
                .find(|t| t.ballot().eq(&ballot))
                .unwrap();
            assert_eq!(
                tally.stake_weights().stake_weight(),
                stake_weights.stake_weight()
            );
        }

        // Test that operator cannot vote again
        let new_ballot = Ballot::new(WeatherStatus::Cloudy as u8);
        let new_slot = current_slot + 1;
        let result = ballot_box.cast_vote(
            &operator,
            &new_ballot,
            &stake_weights,
            new_slot,
            valid_slots_after_consensus,
        );
        assert!(matches!(result, Err(NCNProgramError::OperatorAlreadyVoted)));

        // Test can vote after consensus reached but before window expires
        {
            let tally = ballot_box
                .ballot_tallies
                .iter()
                .find(|t| t.ballot().eq(&ballot))
                .unwrap();
            let winning_ballot = *tally.ballot();
            ballot_box.set_winning_ballot(&winning_ballot);
            ballot_box.slot_consensus_reached = PodU64::from(new_slot);
        }

        let new_operator = Pubkey::new_unique();
        ballot_box
            .cast_vote(
                &new_operator,
                &ballot,
                &stake_weights,
                new_slot + 1,
                valid_slots_after_consensus,
            )
            .unwrap();

        // Test voting window expired after consensus
        let result = ballot_box.cast_vote(
            &new_operator,
            &ballot,
            &stake_weights,
            new_slot + valid_slots_after_consensus + 1,
            valid_slots_after_consensus,
        );
        msg!("result: {:?}", result);
        assert!(matches!(result, Err(NCNProgramError::VotingNotValid)));
    }

    #[test]
    fn test_get_winning_ballot() {
        // Create a new ballot box (should have no winning ballot)
        let ballot_box = BallotBox::new(&Pubkey::default(), 0, 0, 0);

        // Test with no winning ballot initialized
        let result = ballot_box.get_winning_ballot();
        assert_eq!(
            result,
            Err(NCNProgramError::ConsensusNotReached),
            "Should return ConsensusNotReached when no winning ballot is set"
        );

        // Create a new ballot box and set a winning ballot
        let mut ballot_box = BallotBox::new(&Pubkey::default(), 0, 0, 0);
        let expected_ballot = Ballot::new(WeatherStatus::Cloudy as u8);
        ballot_box.set_winning_ballot(&expected_ballot);

        // Test with winning ballot set
        let result = ballot_box.get_winning_ballot();
        assert!(result.is_ok(), "Should succeed when winning ballot is set");
        assert_eq!(
            result.unwrap(),
            &expected_ballot,
            "Should return the correct winning ballot"
        );
    }

    #[test]
    fn test_operator_votes_full() {
        let current_slot = 100;
        let epoch = 1;
        let valid_slots_after_consensus = 10;
        let mut ballot_box = BallotBox::new(&Pubkey::default(), epoch, 0, current_slot);
        let ballot = Ballot::new(WeatherStatus::Sunny as u8);
        let stake_weights = StakeWeights::new(1000);

        // Fill up all operator vote slots (MAX_OPERATORS = 256)
        for _ in 0..MAX_OPERATORS {
            let operator = Pubkey::new_unique();
            let result = ballot_box.cast_vote(
                &operator,
                &ballot,
                &stake_weights,
                current_slot,
                valid_slots_after_consensus,
            );
            assert!(result.is_ok(), "Vote should succeed when slots available");
        }

        // Try to add one more vote, which should fail
        let extra_operator = Pubkey::new_unique();
        let result = ballot_box.cast_vote(
            &extra_operator,
            &ballot,
            &stake_weights,
            current_slot,
            valid_slots_after_consensus,
        );
        assert_eq!(
            result,
            Err(NCNProgramError::OperatorVotesFull),
            "Should return OperatorVotesFull when no slots available"
        );
    }

    #[test]
    fn test_increment_or_create_ballot_tally() {
        let mut ballot_box = BallotBox::new(&Pubkey::new_unique(), 1, 1, 1);
        let ballot = Ballot::new(WeatherStatus::Sunny as u8);
        let stake_weights = StakeWeights::new(100);

        // Test creating new ballot tally
        let tally_index = ballot_box
            .increment_or_create_ballot_tally(&ballot, &stake_weights)
            .unwrap();
        assert_eq!(tally_index, 0);
        assert_eq!(ballot_box.unique_ballots(), 1);
        assert_eq!(
            ballot_box.ballot_tallies[0].stake_weights().stake_weight(),
            stake_weights.stake_weight()
        );
        assert_eq!(*ballot_box.ballot_tallies[0].ballot(), ballot);

        // Test incrementing existing ballot tally
        let tally_index = ballot_box
            .increment_or_create_ballot_tally(&ballot, &stake_weights)
            .unwrap();
        assert_eq!(tally_index, 0);
        assert_eq!(ballot_box.unique_ballots(), 1);
        assert_eq!(
            ballot_box.ballot_tallies[0].stake_weights().stake_weight(),
            stake_weights.stake_weight() * 2
        );
        assert_eq!(*ballot_box.ballot_tallies[0].ballot(), ballot);

        // Test creating second ballot tally
        let ballot2 = Ballot::new(WeatherStatus::Cloudy as u8);
        let tally_index = ballot_box
            .increment_or_create_ballot_tally(&ballot2, &stake_weights)
            .unwrap();
        assert_eq!(tally_index, 1);
        assert_eq!(ballot_box.unique_ballots(), 2);
        assert_eq!(
            ballot_box.ballot_tallies[1].stake_weights().stake_weight(),
            stake_weights.stake_weight()
        );
        assert_eq!(*ballot_box.ballot_tallies[1].ballot(), ballot2);
    }

    #[test]
    fn test_tally_votes() {
        let ncn = Pubkey::new_unique();
        let current_slot = 100;
        let epoch = 1;
        let quarter_stake_weights = StakeWeights::new(500);
        let half_stake_weights = StakeWeights::new(500);
        let full_stake_weights = StakeWeights::new(1000);
        let total_stake_weight: u128 = 1000;
        let mut ballot_box = BallotBox::new(&ncn, epoch, 0, current_slot);
        let ballot = Ballot::new(WeatherStatus::Sunny as u8);

        // Test no consensus when below threshold
        ballot_box
            .increment_or_create_ballot_tally(&ballot, &half_stake_weights)
            .unwrap();
        ballot_box
            .tally_votes(total_stake_weight, current_slot)
            .unwrap();
        assert!(!ballot_box.is_consensus_reached());
        assert_eq!(
            ballot_box.slot_consensus_reached(),
            DEFAULT_CONSENSUS_REACHED_SLOT
        );
        assert!(matches!(
            ballot_box.get_winning_ballot_tally(),
            Err(NCNProgramError::ConsensusNotReached)
        ));

        // Test consensus reached when above threshold
        ballot_box
            .increment_or_create_ballot_tally(&ballot, &half_stake_weights)
            .unwrap();
        ballot_box
            .tally_votes(total_stake_weight, current_slot)
            .unwrap();
        assert!(ballot_box.is_consensus_reached());
        assert_eq!(ballot_box.slot_consensus_reached(), current_slot);
        assert_eq!(
            *ballot_box.get_winning_ballot_tally().unwrap().ballot(),
            ballot
        );

        // Consensus remains after additional votes
        let ballot2 = Ballot::new(WeatherStatus::Sunny as u8);
        ballot_box
            .increment_or_create_ballot_tally(&ballot2, &full_stake_weights)
            .unwrap();
        ballot_box
            .tally_votes(total_stake_weight, current_slot + 1)
            .unwrap();
        assert!(ballot_box.is_consensus_reached());
        assert_eq!(ballot_box.slot_consensus_reached(), current_slot);
        assert_eq!(
            *ballot_box.get_winning_ballot_tally().unwrap().ballot(),
            ballot
        );

        // Test with multiple competing ballots
        let mut ballot_box = BallotBox::new(&ncn, epoch, 0, current_slot);
        let ballot1 = Ballot::new(WeatherStatus::Sunny as u8);
        let ballot2 = Ballot::new(WeatherStatus::Cloudy as u8);
        let ballot3 = Ballot::new(WeatherStatus::Rainy as u8);

        ballot_box
            .increment_or_create_ballot_tally(&ballot1, &quarter_stake_weights)
            .unwrap();
        ballot_box
            .increment_or_create_ballot_tally(&ballot2, &quarter_stake_weights)
            .unwrap();
        ballot_box
            .increment_or_create_ballot_tally(&ballot3, &half_stake_weights)
            .unwrap();

        ballot_box
            .tally_votes(total_stake_weight, current_slot)
            .unwrap();
        assert!(!ballot_box.is_consensus_reached());

        // Add more votes to reach consensus
        ballot_box
            .increment_or_create_ballot_tally(&ballot3, &half_stake_weights)
            .unwrap();
        ballot_box
            .tally_votes(total_stake_weight, current_slot)
            .unwrap();
        assert!(ballot_box.is_consensus_reached());
        assert_eq!(
            *ballot_box.get_winning_ballot_tally().unwrap().ballot(),
            ballot3
        );
    }

    #[test]
    fn test_cast_bad_ballot() {
        let ncn = Pubkey::new_unique();
        let current_slot = 100;
        let epoch = 1;
        let valid_slots_after_consensus = 10;
        let stake_weight_per_operator = 1000;
        let mut ballot_box = BallotBox::new(&ncn, epoch, 0, current_slot);

        let operator1 = Pubkey::new_unique();

        let stake_weights = StakeWeights::new(stake_weight_per_operator);
        let ballot1 = Ballot::new(99);

        // Operator 1 votes for ballot1 initially
        let result = ballot_box.cast_vote(
            &operator1,
            &ballot1,
            &stake_weights,
            current_slot,
            valid_slots_after_consensus,
        );

        msg!("result: {:?}", result);
        assert_ncn_program_error(result, NCNProgramError::BadBallot);
    }

    #[test]
    fn test_multiple_operators_converging_votes() {
        let ncn = Pubkey::new_unique();
        let current_slot = 100;
        let epoch = 1;
        let valid_slots_after_consensus = 10;
        let stake_weight_per_operator = 1000;
        let mut ballot_box = BallotBox::new(&ncn, epoch, 0, current_slot);

        let operator1 = Pubkey::new_unique();
        let operator2 = Pubkey::new_unique();
        let operator3 = Pubkey::new_unique();

        let stake_weights = StakeWeights::new(stake_weight_per_operator);
        let ballot1 = Ballot::new(WeatherStatus::Sunny as u8);
        let ballot2 = Ballot::new(WeatherStatus::Cloudy as u8);

        // Operator 1 votes for ballot1 initially
        ballot_box
            .cast_vote(
                &operator1,
                &ballot1,
                &stake_weights,
                current_slot,
                valid_slots_after_consensus,
            )
            .unwrap();

        assert_eq!(ballot_box.unique_ballots(), 1);

        // Operator 2 votes for ballot2
        ballot_box
            .cast_vote(
                &operator2,
                &ballot2,
                &stake_weights,
                current_slot + 2,
                valid_slots_after_consensus,
            )
            .unwrap();
        assert_eq!(ballot_box.unique_ballots(), 2);

        // Operator 3 votes for ballot2
        ballot_box
            .cast_vote(
                &operator3,
                &ballot2,
                &stake_weights,
                current_slot + 3,
                valid_slots_after_consensus,
            )
            .unwrap();

        assert_eq!(ballot_box.unique_ballots(), 2);

        // Check total stake weight
        let winning_tally = ballot_box
            .ballot_tallies
            .iter()
            .find(|t| t.ballot().eq(&ballot2))
            .unwrap();

        assert_eq!(winning_tally.stake_weights().stake_weight(), 2000);
        assert_eq!(winning_tally.tally(), 2);

        // Verify ballot2 wins consensus with all votes
        ballot_box.tally_votes(2000, current_slot + 4).unwrap();
        assert!(ballot_box.has_winning_ballot());
        assert_eq!(*ballot_box.get_winning_ballot().unwrap(), ballot2);
    }

    #[test]
    fn test_set_tie_breaker_ballot() {
        let ncn = Pubkey::new_unique();
        let current_slot = 100;
        let epoch = 1;
        let mut ballot_box = BallotBox::new(&ncn, epoch, 0, current_slot);

        // Create some initial ballots
        let ballot1 = Ballot::new(WeatherStatus::Sunny as u8);
        let ballot2 = Ballot::new(WeatherStatus::Cloudy as u8);
        let stake_weights = StakeWeights::new(100);
        let double_stake_weights = StakeWeights::new(200);

        ballot_box
            .increment_or_create_ballot_tally(&ballot1, &stake_weights)
            .unwrap();
        ballot_box
            .increment_or_create_ballot_tally(&ballot2, &double_stake_weights)
            .unwrap();

        // Test setting tie breaker before voting is stalled
        let current_epoch = epoch + 1;
        let epochs_before_stall = 3;

        assert_eq!(
            ballot_box.set_tie_breaker_ballot(
                WeatherStatus::Sunny as u8,
                current_epoch,
                epochs_before_stall,
            ),
            Err(NCNProgramError::VotingNotFinalized)
        );

        // Test setting tie breaker with invalid weather status
        assert_eq!(
            ballot_box.set_tie_breaker_ballot(
                (WeatherStatus::Rainy as u8) + 1,
                current_epoch + epochs_before_stall,
                epochs_before_stall,
            ),
            Err(NCNProgramError::BadBallot)
        );

        // Test setting tie breaker with non-existent ballot
        assert_eq!(
            ballot_box.set_tie_breaker_ballot(
                WeatherStatus::Rainy as u8,
                current_epoch + epochs_before_stall,
                epochs_before_stall,
            ),
            Err(NCNProgramError::TieBreakerNotInPriorVotes)
        );

        // Test successful tie breaker setting
        let current_epoch = epoch + epochs_before_stall;
        ballot_box
            .set_tie_breaker_ballot(
                WeatherStatus::Sunny as u8,
                current_epoch,
                epochs_before_stall,
            )
            .unwrap();
        assert!(ballot_box.is_consensus_reached());
        assert_eq!(ballot_box.get_winning_ballot().unwrap(), &ballot1);
    }

    #[test]
    fn test_operator_cannot_vote_twice() {
        let ncn = Pubkey::new_unique();
        let operator = Pubkey::new_unique();
        let current_slot = 100;
        let epoch = 1;
        let stake_weights = StakeWeights::new(1000);
        let valid_slots_after_consensus = 10;
        let mut ballot_box = BallotBox::new(&ncn, epoch, 0, current_slot);

        // First vote should succeed
        let ballot1 = Ballot::new(WeatherStatus::Sunny as u8);
        ballot_box
            .cast_vote(
                &operator,
                &ballot1,
                &stake_weights,
                current_slot,
                valid_slots_after_consensus,
            )
            .unwrap();

        // Verify first vote was recorded
        {
            let operator_vote = ballot_box
                .operator_votes
                .iter()
                .find(|v| v.operator().eq(&operator))
                .unwrap();
            assert_eq!(operator_vote.ballot_index(), 0);
            assert_eq!(
                operator_vote.stake_weights().stake_weight(),
                stake_weights.stake_weight()
            );

            let ballot_tally = ballot_box
                .ballot_tallies
                .iter()
                .find(|t| t.ballot().eq(&ballot1))
                .unwrap();
            assert_eq!(ballot_tally.tally(), 1);
            assert_eq!(
                ballot_tally.stake_weights().stake_weight(),
                stake_weights.stake_weight()
            );
        }

        // Second vote should fail
        let ballot2 = Ballot::new(WeatherStatus::Cloudy as u8);
        let result = ballot_box.cast_vote(
            &operator,
            &ballot2,
            &stake_weights,
            current_slot + 1,
            valid_slots_after_consensus,
        );
        assert!(matches!(result, Err(NCNProgramError::OperatorAlreadyVoted)));

        // Verify first vote remains unchanged
        {
            let operator_vote = ballot_box
                .operator_votes
                .iter()
                .find(|v| v.operator().eq(&operator))
                .unwrap();
            assert_eq!(operator_vote.ballot_index(), 0);
            assert_eq!(operator_vote.slot_voted(), current_slot);

            let ballot1_tally = ballot_box
                .ballot_tallies
                .iter()
                .find(|t| t.ballot().eq(&ballot1))
                .unwrap();
            assert_eq!(ballot1_tally.tally(), 1);

            // Verify ballot2 was not recorded
            assert!(ballot_box
                .ballot_tallies
                .iter()
                .find(|t| t.ballot().eq(&ballot2))
                .is_none());
        }

        // Verify total counts
        assert_eq!(ballot_box.operators_voted(), 1);
        assert_eq!(ballot_box.unique_ballots(), 1);
    }
}

#[cfg(test)]
mod zero_stake_tests {
    use super::*;

    #[test]
    fn test_zero_stake_operator_basic_voting() {
        let ncn = Pubkey::new_unique();
        let current_slot = 100;
        let epoch = 1;
        let valid_slots_after_consensus = 100;
        let mut ballot_box = BallotBox::new(&ncn, epoch, 0, current_slot);

        // Create ballots and operators
        let ballot = Ballot::new(WeatherStatus::default() as u8);

        let zero_stake_operator = Pubkey::new_unique();
        let zero_stake = StakeWeights::new(0);

        // Zero stake operator can cast a vote
        ballot_box
            .cast_vote(
                &zero_stake_operator,
                &ballot,
                &zero_stake,
                current_slot,
                valid_slots_after_consensus,
            )
            .unwrap();

        // Verify vote was recorded
        let operator_vote = ballot_box
            .operator_votes()
            .iter()
            .find(|v| v.operator().eq(&zero_stake_operator))
            .expect("Zero stake operator vote should be recorded");

        assert_eq!(operator_vote.stake_weights().stake_weight(), 0);

        // Verify ballot tally
        let ballot_tally = ballot_box
            .ballot_tallies()
            .iter()
            .find(|t| t.ballot().eq(&ballot))
            .expect("Ballot tally should exist");

        assert_eq!(ballot_tally.stake_weights().stake_weight(), 0);
        assert_eq!(ballot_tally.tally(), 1);
    }

    #[test]
    fn test_zero_stake_operator_consensus() {
        let ncn = Pubkey::new_unique();
        let current_slot = 100;
        let epoch = 1;
        let valid_slots_after_consensus = 100;
        let mut ballot_box = BallotBox::new(&ncn, epoch, 0, current_slot);

        let ballot = Ballot::new(WeatherStatus::default() as u8);

        // Create multiple zero stake operators
        let num_zero_stake = 5;
        let zero_stake_operators: Vec<Pubkey> =
            (0..num_zero_stake).map(|_| Pubkey::new_unique()).collect();
        let zero_stake = StakeWeights::new(0);

        // Have all zero stake operators vote for the same ballot
        for (i, operator) in zero_stake_operators.iter().enumerate() {
            ballot_box
                .cast_vote(
                    operator,
                    &ballot,
                    &zero_stake,
                    current_slot + i as u64,
                    valid_slots_after_consensus,
                )
                .unwrap();
        }

        // Check ballot state after zero stake votes
        let ballot_tally = ballot_box
            .ballot_tallies()
            .iter()
            .find(|t| t.ballot().eq(&ballot))
            .expect("Ballot tally should exist");

        assert_eq!(ballot_tally.stake_weights().stake_weight(), 0);
        assert_eq!(ballot_tally.tally(), num_zero_stake as u64);

        // Calculate consensus with only zero stake votes
        let total_stake = 1000u128;
        ballot_box
            .tally_votes(total_stake, current_slot + num_zero_stake as u64)
            .unwrap();
        assert!(
            !ballot_box.is_consensus_reached(),
            "Zero stake votes alone should not reach consensus"
        );

        // Add one normal stake vote
        let normal_operator = Pubkey::new_unique();
        let normal_stake = StakeWeights::new(700); // 70% of total stake

        ballot_box
            .cast_vote(
                &normal_operator,
                &ballot,
                &normal_stake,
                current_slot + num_zero_stake as u64,
                valid_slots_after_consensus,
            )
            .unwrap();

        // Verify ballot tally includes both zero and normal stakes
        let ballot_tally = ballot_box
            .ballot_tallies()
            .iter()
            .find(|t| t.ballot().eq(&ballot))
            .expect("Ballot tally should exist");

        assert_eq!(
            ballot_tally.stake_weights().stake_weight(),
            normal_stake.stake_weight()
        );
        assert_eq!(ballot_tally.tally(), (num_zero_stake + 1) as u64);

        // Check consensus again
        ballot_box
            .tally_votes(total_stake, current_slot + num_zero_stake as u64 + 1)
            .unwrap();
        assert!(
            ballot_box.is_consensus_reached(),
            "Consensus should be reached with normal stake vote despite zero stake votes"
        );
    }

    #[test]
    fn test_zero_stake_operator_mixed_voting() {
        let ncn = Pubkey::new_unique();
        let current_slot = 100;
        let epoch = 1;
        let valid_slots_after_consensus = 100;
        let mut ballot_box = BallotBox::new(&ncn, epoch, 0, current_slot);

        let ballot1 = Ballot::new(WeatherStatus::Sunny as u8);
        let ballot2 = Ballot::new(WeatherStatus::Cloudy as u8);

        // Create mix of zero and normal stake operators
        let zero_stake_operator = Pubkey::new_unique();
        let zero_stake = StakeWeights::new(0);

        let normal_operator1 = Pubkey::new_unique();
        let normal_stake1 = StakeWeights::new(300);

        let normal_operator2 = Pubkey::new_unique();
        let normal_stake2 = StakeWeights::new(400);

        // Cast votes for ballot1
        ballot_box
            .cast_vote(
                &zero_stake_operator,
                &ballot1,
                &zero_stake,
                current_slot,
                valid_slots_after_consensus,
            )
            .unwrap();
        ballot_box
            .cast_vote(
                &normal_operator1,
                &ballot1,
                &normal_stake1,
                current_slot,
                valid_slots_after_consensus,
            )
            .unwrap();

        // Cast vote for ballot2
        ballot_box
            .cast_vote(
                &normal_operator2,
                &ballot2,
                &normal_stake2,
                current_slot,
                valid_slots_after_consensus,
            )
            .unwrap();

        // Verify ballot tallies
        let ballot1_tally = ballot_box
            .ballot_tallies()
            .iter()
            .find(|t| t.ballot().eq(&ballot1))
            .expect("Ballot1 tally should exist");

        assert_eq!(
            ballot1_tally.stake_weights().stake_weight(),
            normal_stake1.stake_weight()
        );
        assert_eq!(ballot1_tally.tally(), 2); // Counts both zero and normal stake votes

        let ballot2_tally = ballot_box
            .ballot_tallies()
            .iter()
            .find(|t| t.ballot().eq(&ballot2))
            .expect("Ballot2 tally should exist");

        assert_eq!(
            ballot2_tally.stake_weights().stake_weight(),
            normal_stake2.stake_weight()
        );
        assert_eq!(ballot2_tally.tally(), 1);

        // Check consensus
        let total_stake = 1000u128;
        ballot_box.tally_votes(total_stake, current_slot).unwrap();

        // Neither ballot should have consensus yet
        assert!(!ballot_box.is_consensus_reached());

        // Add another normal stake vote to ballot2 to reach consensus
        let normal_operator3 = Pubkey::new_unique();
        let normal_stake3 = StakeWeights::new(300);
        ballot_box
            .cast_vote(
                &normal_operator3,
                &ballot2,
                &normal_stake3,
                current_slot,
                valid_slots_after_consensus,
            )
            .unwrap();

        ballot_box.tally_votes(total_stake, current_slot).unwrap();

        assert!(ballot_box.is_consensus_reached());
        assert_eq!(ballot_box.get_winning_ballot().unwrap(), &ballot2);
    }
}
