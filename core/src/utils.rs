use solana_program::{program_error::ProgramError, pubkey::Pubkey};

use crate::{
    ballot_box::BallotBox, constants::MAX_REALLOC_BYTES, epoch_snapshot::OperatorSnapshot,
    error::NCNProgramError,
};

/// Calculate new size for reallocation, capped at target size
/// Returns the minimum of (current_size + MAX_REALLOC_BYTES) and target_size
pub fn get_new_size(current_size: usize, target_size: usize) -> Result<usize, ProgramError> {
    Ok(current_size
        .checked_add(MAX_REALLOC_BYTES as usize)
        .ok_or(ProgramError::ArithmeticOverflow)?
        .min(target_size))
}

#[inline(always)]
#[track_caller]
pub fn assert_ncn_program_error<T>(
    test_error: Result<T, NCNProgramError>,
    ncn_program_error: NCNProgramError,
) {
    assert!(test_error.is_err());
    assert_eq!(test_error.err().unwrap(), ncn_program_error);
}

pub fn can_operator_vote(
    ballot_box: BallotBox,
    operator_snapshot: OperatorSnapshot,
    operator: &Pubkey,
) -> bool {
    let did_operator_vote = ballot_box.did_operator_vote(operator);

    if did_operator_vote {
        return false;
    }

    let stake_weight = operator_snapshot.stake_weights().stake_weight();

    if stake_weight == 0 {
        return false;
    }

    true
}
