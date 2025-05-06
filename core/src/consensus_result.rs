use core::fmt;
use std::mem::size_of;

use bytemuck::{Pod, Zeroable};
use jito_bytemuck::{
    types::{PodBool, PodU64},
    AccountDeserialize, Discriminator,
};
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
    /// Bump seed for the PDA
    bump: u8,
    /// Padding for alignment
    _padding: [u8; 7],
    /// The winning weather status that reached consensus
    weather_status: u8,
    /// Padding for alignment
    _padding1: [u8; 7],
    /// The vote weight that supported the winning status
    vote_weight: PodU64,
    /// The total vote weight in the ballot box
    total_vote_weight: PodU64,
    /// The slot at which consensus was reached
    consensus_slot: PodU64,
    /// The account that recorded the consensus
    consensus_recorder: Pubkey,
    /// Whether consensus has been reached
    consensus_reached: PodBool,
}

impl Discriminator for ConsensusResult {
    const DISCRIMINATOR: u8 = Discriminators::ConsensusResult as u8;
}

impl ConsensusResult {
    pub const SIZE: usize = 8 + size_of::<Self>();

    pub fn new(ncn: &Pubkey, epoch: u64, bump: u8) -> Self {
        Self {
            ncn: *ncn,
            epoch: PodU64::from(epoch),
            bump,
            _padding: [0; 7],
            weather_status: 0,
            _padding1: [0; 7],
            vote_weight: PodU64::from(0),
            total_vote_weight: PodU64::from(0),
            consensus_slot: PodU64::from(0),
            consensus_recorder: Pubkey::default(),
            consensus_reached: PodBool::from(false),
        }
    }

    pub fn seeds(ncn: &Pubkey, epoch: u64) -> Vec<Vec<u8>> {
        Vec::from_iter(
            [
                b"consensus-result".to_vec(),
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

    pub const fn consensus_recorder(&self) -> &Pubkey {
        &self.consensus_recorder
    }

    pub fn is_consensus_reached(&self) -> bool {
        self.consensus_reached.into()
    }

    pub fn record_consensus(
        &mut self,
        weather_status: u8,
        vote_weight: u64,
        total_vote_weight: u64,
        consensus_slot: u64,
        consensus_recorder: &Pubkey,
    ) -> Result<(), NCNProgramError> {
        if self.is_consensus_reached() {
            self.vote_weight = PodU64::from(vote_weight);
        } else {
            self.weather_status = weather_status;
            self.vote_weight = PodU64::from(vote_weight);
            self.total_vote_weight = PodU64::from(total_vote_weight);
            self.consensus_slot = PodU64::from(consensus_slot);
            self.consensus_recorder = *consensus_recorder;
            self.consensus_reached = PodBool::from(true);
        }

        Ok(())
    }

    pub fn initialize(&mut self, ncn: &Pubkey, epoch: u64, bump: u8) -> Result<(), ProgramError> {
        self.ncn = *ncn;
        self.epoch = PodU64::from(epoch);
        self.bump = bump;
        self.weather_status = 0;
        self.vote_weight = PodU64::from(0);
        self.total_vote_weight = PodU64::from(0);
        self.consensus_slot = PodU64::from(0);
        self.consensus_recorder = Pubkey::default();
        self.consensus_reached = PodBool::from(false);

        Ok(())
    }
}

impl fmt::Display for ConsensusResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ConsensusResult {{ ncn: {}, epoch: {}, weather_status: {}, vote_weight: {}, total_vote_weight: {}, consensus_slot: {}, consensus_recorder: {}, consensus_reached: {} }}",
            self.ncn,
            self.epoch(),
            self.weather_status,
            self.vote_weight(),
            self.total_vote_weight(),
            self.consensus_slot(),
            self.consensus_recorder,
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
        assert_eq!(consensus_result.consensus_recorder(), &Pubkey::default());

        let recorder = Pubkey::new_unique();
        consensus_result
            .record_consensus(2, 1000, 2000, 5000, &recorder)
            .unwrap();

        assert!(consensus_result.is_consensus_reached());
        assert_eq!(consensus_result.weather_status(), 2);
        assert_eq!(consensus_result.vote_weight(), 1000);
        assert_eq!(consensus_result.total_vote_weight(), 2000);
        assert_eq!(consensus_result.consensus_slot(), 5000);
        assert_eq!(consensus_result.consensus_recorder(), &recorder);
    }

    #[test]
    fn test_find_program_address() {
        let program_id = Pubkey::new_unique();
        let ncn = Pubkey::new_unique();
        let epoch = 123;

        let (pda, bump, seeds) = ConsensusResult::find_program_address(&program_id, &ncn, epoch);

        assert_eq!(seeds.len(), 3);
        assert_eq!(seeds[0], b"consensus-result".to_vec());
        assert_eq!(seeds[1], ncn.to_bytes().to_vec());
        assert_eq!(seeds[2], epoch.to_le_bytes().to_vec());
    }
}
