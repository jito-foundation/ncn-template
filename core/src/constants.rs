use solana_program::{clock::DEFAULT_SLOTS_PER_EPOCH, entrypoint::MAX_PERMITTED_DATA_INCREASE};
use spl_math::precise_number::PreciseNumber;

use crate::error::NCNProgramError;

pub const MAX_FEE_BPS: u64 = 10_000;
pub const MAX_ST_MINTS: usize = 64;
pub const MAX_VAULTS: usize = 64;
pub const MAX_OPERATORS: usize = 256;
pub const MIN_EPOCHS_BEFORE_STALL: u64 = 1;
pub const MAX_EPOCHS_BEFORE_STALL: u64 = 50;
pub const MIN_EPOCHS_AFTER_CONSENSUS_BEFORE_CLOSE: u64 = 10;
pub const MAX_EPOCHS_AFTER_CONSENSUS_BEFORE_CLOSE: u64 = 100;
pub const MIN_VALID_SLOTS_AFTER_CONSENSUS: u64 = 1000;
pub const MAX_VALID_SLOTS_AFTER_CONSENSUS: u64 = 50 * DEFAULT_SLOTS_PER_EPOCH;
const PRECISE_CONSENSUS_NUMERATOR: u128 = 2;
const PRECISE_CONSENSUS_DENOMINATOR: u128 = 3;
pub fn precise_consensus() -> Result<PreciseNumber, NCNProgramError> {
    PreciseNumber::new(PRECISE_CONSENSUS_NUMERATOR)
        .ok_or(NCNProgramError::NewPreciseNumberError)?
        .checked_div(
            &PreciseNumber::new(PRECISE_CONSENSUS_DENOMINATOR)
                .ok_or(NCNProgramError::NewPreciseNumberError)?,
        )
        .ok_or(NCNProgramError::DenominatorIsZero)
}

pub const DEFAULT_CONSENSUS_REACHED_SLOT: u64 = u64::MAX;
pub const MAX_REALLOC_BYTES: u64 = MAX_PERMITTED_DATA_INCREASE as u64;

pub const WEIGHT: u128 = 100;
