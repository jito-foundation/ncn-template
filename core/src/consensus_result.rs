// Consensus Result Module
//
// This module implements the ConsensusResult account type, which stores the final outcome 
// of a voting process for a particular epoch. Once consensus is reached in the BallotBox,
// the winning result is recorded in this account.
//
// The ConsensusResult account serves as a permanent record of:
// - Which ballot option won the consensus
// - How much voting weight supported the winning option
// - The total voting weight of all participants
// - When (at which slot) consensus was reached
//
// This account can be queried by other parts of the protocol or external systems
// to determine the most recent consensus state.

use core::fmt;
use std::mem::size_of;

use bytemuck::{Pod, Zeroable};
use jito_bytemuck::{types::PodU64, AccountDeserialize, Discriminator};
use shank::ShankAccount;
use solana_program::{account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey};

use crate::{discriminators::Discriminators, error::NCNProgramError, loaders::check_load};

// PDA'd ["consensus-result", NCN, NCN_EPOCH_SLOT]
#[derive(Debug, Clone, Copy, Zeroable, Pod, AccountDeserialize, ShankAccount)]
#[repr(C)]
pub struct ConsensusResult {
    /// The NCN this consensus result is for
    ncn: Pubkey,
    /// The epoch this consensus result is for
    epoch: PodU64,
    /// The vote weight that supported the winning status
    vote_weight: PodU64,
    /// The total vote weight in the ballot box
    total_vote_weight: PodU64,
    /// The slot at which consensus was reached
    consensus_slot: PodU64,
    /// Bump seed for the PDA
    bump: u8,
    /// The winning weather status that reached consensus
    weather_status: u8,
}

impl Discriminator for ConsensusResult {
    const DISCRIMINATOR: u8 = Discriminators::ConsensusResult as u8;
}

impl ConsensusResult {
    const CONSENSUS_RESULT_SEED: &'static [u8] = b"consensus-result";
    pub const SIZE: usize = 8 + size_of::<Self>();

    /// Creates a new ConsensusResult instance with default values
    ///
    /// # Arguments
    /// * `ncn` - The NCN pubkey this result is associated with
    /// * `epoch` - The epoch number for this consensus round
    /// * `bump` - PDA bump seed
    pub fn new(ncn: &Pubkey, epoch: u64, bump: u8) -> Self {
        Self {
            ncn: *ncn,
            epoch: PodU64::from(epoch),
            bump,
            weather_status: 0,
            vote_weight: PodU64::from(0),
            total_vote_weight: PodU64::from(0),
            consensus_slot: PodU64::from(0),
        }
    }

    pub fn seeds(ncn: &Pubkey, epoch: u64) -> Vec<Vec<u8>> {
        Vec::from_iter(
            [
                Self::CONSENSUS_RESULT_SEED.to_vec(),
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

    /// Validates that the provided account matches the expected PDA and has the right discriminator
    ///
    /// # Arguments
    /// * `program_id` - The program ID
    /// * `account` - The account to validate
    /// * `ncn` - The NCN pubkey
    /// * `epoch` - The epoch number
    /// * `expect_writable` - Whether the account should be writable
    ///
    /// # Returns
    /// * `Result<(), ProgramError>` - Ok if valid, Error otherwise
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

    pub fn epoch(&self) -> u64 {
        self.epoch.into()
    }

    pub fn consensus_slot(&self) -> u64 {
        self.consensus_slot.into()
    }

    pub const fn ncn(&self) -> &Pubkey {
        &self.ncn
    }

    pub fn weather_status(&self) -> u8 {
        self.weather_status
    }

    pub fn vote_weight(&self) -> u64 {
        self.vote_weight.into()
    }

    pub fn total_vote_weight(&self) -> u64 {
        self.total_vote_weight.into()
    }

    pub fn is_consensus_reached(&self) -> bool {
        self.consensus_slot != PodU64::from(0)
    }

    /// Records the consensus result data when consensus is reached
    ///
    /// # Arguments
    /// * `weather_status` - The winning weather status
    /// * `vote_weight` - The vote weight that supported the winning status
    /// * `total_vote_weight` - The total vote weight
    /// * `consensus_slot` - The slot when consensus was reached
    ///
    /// # Returns
    /// * `Result<(), NCNProgramError>` - Ok if successful
    pub fn record_consensus(
        &mut self,
        weather_status: u8,
        vote_weight: u64,
        total_vote_weight: u64,
        consensus_slot: u64,
    ) -> Result<(), NCNProgramError> {
        if self.is_consensus_reached() {
            self.vote_weight = PodU64::from(vote_weight);
        } else {
            self.weather_status = weather_status;
            self.vote_weight = PodU64::from(vote_weight);
            self.total_vote_weight = PodU64::from(total_vote_weight);
            self.consensus_slot = PodU64::from(consensus_slot);
        }

        Ok(())
    }

    /// Initializes the ConsensusResult account with default values
    ///
    /// # Arguments
    /// * `ncn` - The NCN pubkey
    /// * `epoch` - The epoch number
    /// * `bump` - PDA bump seed
    ///
    /// # Returns
    /// * `Result<(), ProgramError>` - Ok if successful
    pub fn initialize(&mut self, ncn: &Pubkey, epoch: u64, bump: u8) -> Result<(), ProgramError> {
        self.ncn = *ncn;
        self.epoch = PodU64::from(epoch);
        self.bump = bump;
        self.weather_status = 0;
        self.vote_weight = PodU64::from(0);
        self.total_vote_weight = PodU64::from(0);
        self.consensus_slot = PodU64::from(0);

        Ok(())
    }
}

impl fmt::Display for ConsensusResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ConsensusResult {{ ncn: {}, epoch: {}, weather_status: {}, vote_weight: {}, total_vote_weight: {}, consensus_slot: {},  consensus_reached: {} }}",
            self.ncn,
            self.epoch(),
            self.weather_status,
            self.vote_weight(),
            self.total_vote_weight(),
            self.consensus_slot(),
            self.is_consensus_reached(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_program::pubkey::Pubkey;

    #[test]
    fn test_record_consensus() {
        let mut consensus_result = ConsensusResult::new(&Pubkey::new_unique(), 123, 255);

        assert!(!consensus_result.is_consensus_reached());
        assert_eq!(consensus_result.weather_status(), 0);
        assert_eq!(consensus_result.vote_weight(), 0);
        assert_eq!(consensus_result.total_vote_weight(), 0);
        assert_eq!(consensus_result.consensus_slot(), 0);

        consensus_result
            .record_consensus(2, 1000, 2000, 5000)
            .unwrap();

        assert!(consensus_result.is_consensus_reached());
        assert_eq!(consensus_result.weather_status(), 2);
        assert_eq!(consensus_result.vote_weight(), 1000);
        assert_eq!(consensus_result.total_vote_weight(), 2000);
        assert_eq!(consensus_result.consensus_slot(), 5000);
    }

    #[test]
    fn test_find_program_address() {
        let program_id = Pubkey::new_unique();
        let ncn = Pubkey::new_unique();
        let epoch = 123;

        let (_, _, seeds) = ConsensusResult::find_program_address(&program_id, &ncn, epoch);

        assert_eq!(seeds.len(), 3);
        assert_eq!(seeds[0], ConsensusResult::CONSENSUS_RESULT_SEED.to_vec());
        assert_eq!(seeds[1], ncn.to_bytes().to_vec());
        assert_eq!(seeds[2], epoch.to_le_bytes().to_vec());
    }
}
