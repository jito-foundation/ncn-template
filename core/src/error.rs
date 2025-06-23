use solana_program::{decode_error::DecodeError, program_error::ProgramError};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum NCNProgramError {
    #[error("No valid Ballot")]
    NoValidBallots,
    #[error("Zero in the denominator")]
    DenominatorIsZero = 0x2100,
    #[error("Overflow")]
    ArithmeticOverflow,
    #[error("Underflow")]
    ArithmeticUnderflowError,
    #[error("Floor Overflow")]
    ArithmeticFloorError,
    #[error("Modulo Overflow")]
    ModuloOverflow,
    #[error("New precise number error")]
    NewPreciseNumberError,
    #[error("Cast to imprecise number error")]
    CastToImpreciseNumberError,
    #[error("Cast to u64 error")]
    CastToU64Error,
    #[error("Cast to u128 error")]
    CastToU128Error,

    #[error("Incorrect weight table admin")]
    IncorrectWeightTableAdmin = 0x2200,
    #[error("Duplicate mints in table")]
    DuplicateMintsInTable,
    #[error("There are no mints in the table")]
    NoMintsInTable,
    #[error("Table not initialized")]
    TableNotInitialized,
    #[error("Registry not initialized")]
    RegistryNotInitialized,
    #[error("There are no vaults in the registry")]
    NoVaultsInRegistry,
    #[error("Vault not in weight table registry")]
    VaultNotInRegistry,
    #[error("Mint is already in the table")]
    MintInTable,
    #[error("Too many mints for table")]
    TooManyMintsForTable,
    #[error("Too many vaults for registry")]
    TooManyVaultsForRegistry,
    #[error("Weight table already initialized")]
    WeightTableAlreadyInitialized,
    #[error("Cannnot create future weight tables")]
    CannotCreateFutureWeightTables,
    #[error("Weight mints do not match - length")]
    WeightMintsDoNotMatchLength,
    #[error("Weight mints do not match - mint hash")]
    WeightMintsDoNotMatchMintHash,
    #[error("Invalid mint for weight table")]
    InvalidMintForWeightTable,
    #[error("Config supported mints do not match NCN Vault Count")]
    ConfigMintsNotUpdated,
    #[error("NCN config vaults are at capacity")]
    ConfigMintListFull,
    #[error("Vault Registry mints are at capacity")]
    VaultRegistryListFull,
    #[error("Vault registry are locked for the epoch")]
    VaultRegistryVaultLocked,
    #[error("Vault index already in use by a different mint")]
    VaultIndexAlreadyInUse,
    #[error("Mint Entry not found")]
    MintEntryNotFound,
    #[error("Fee cap exceeded")]
    FeeCapExceeded,
    #[error("Total fees cannot be 0")]
    TotalFeesCannotBeZero,
    #[error("Protocol wallet cannot be default")]
    DefaultDaoWallet,
    #[error("NCN Fee wallet cannot be default")]
    DefaultNcnWallet,
    #[error("Incorrect NCN Admin")]
    IncorrectNcnAdmin,
    #[error("Incorrect NCN")]
    IncorrectNcn,
    #[error("Incorrect fee admin")]
    IncorrectFeeAdmin,
    #[error("Weight table not finalized")]
    WeightTableNotFinalized,
    #[error("Weight not found")]
    WeightNotFound,
    #[error("No operators in ncn")]
    NoOperators,
    #[error("Vault operator delegation is already finalized - should not happen")]
    VaultOperatorDelegationFinalized,
    #[error("Operator is already finalized - should not happen")]
    OperatorFinalized,
    #[error("Too many vault operator delegations")]
    TooManyVaultOperatorDelegations,
    #[error("Duplicate vault operator delegation")]
    DuplicateVaultOperatorDelegation,
    #[error("Duplicate Vote Cast")]
    DuplicateVoteCast,
    #[error("Cannot Vote With Zero Delegation")]
    CannotVoteWithZeroStake,
    #[error("Operator Already Voted")]
    OperatorAlreadyVoted,
    #[error("Operator votes full")]
    OperatorVotesFull,
    #[error("Merkle root tally full")]
    BallotTallyFull,
    #[error("Ballot tally not found")]
    BallotTallyNotFoundFull,
    #[error("Ballot tally not empty")]
    BallotTallyNotEmpty,
    #[error("Consensus already reached, cannot change vote")]
    ConsensusAlreadyReached,
    #[error("Consensus not reached")]
    ConsensusNotReached,

    #[error("Epoch snapshot not finalized")]
    EpochSnapshotNotFinalized,
    #[error("Voting not valid, too many slots after consensus reached")]
    VotingNotValid,
    #[error("Tie breaker admin invalid")]
    TieBreakerAdminInvalid,
    #[error("Voting not finalized")]
    VotingNotFinalized,
    #[error("Tie breaking ballot must be one of the prior votes")]
    TieBreakerNotInPriorVotes,
    #[error("Invalid merkle proof")]
    InvalidMerkleProof,
    #[error("Operator voter needs to sign its vote")]
    InvalidOperatorVoter,
    #[error("Not a valid NCN fee group")]
    InvalidNcnFeeGroup,
    #[error("Not a valid base fee group")]
    InvalidBaseFeeGroup,
    #[error("Operator reward list full")]
    OperatorRewardListFull,
    #[error("Operator Reward not found")]
    OperatorRewardNotFound,
    #[error("Vault Reward not found")]
    VaultRewardNotFound,
    #[error("Destination mismatch")]
    DestinationMismatch,
    #[error("Ncn reward route not found")]
    NcnRewardRouteNotFound,
    #[error("Fee not active")]
    FeeNotActive,
    #[error("No rewards to distribute")]
    NoRewards,
    #[error("Weight not set")]
    WeightNotSet,
    #[error("Router still routing")]
    RouterStillRouting,
    #[error("Invalid epochs before stall")]
    InvalidEpochsBeforeStall,
    #[error("Invalid epochs before accounts can close")]
    InvalidEpochsBeforeClose,
    #[error("Invalid slots after consensus")]
    InvalidSlotsAfterConsensus,
    #[error("Vault needs to be updated")]
    VaultNeedsUpdate,
    #[error("Invalid Account Status")]
    InvalidAccountStatus,
    #[error("Account already initialized")]
    AccountAlreadyInitialized,
    #[error("Cannot vote with uninitialized account")]
    BadBallot,
    #[error("Cannot route until voting is over")]
    VotingIsNotOver,
    #[error("Operator is not in snapshot")]
    OperatorIsNotInSnapshot,
    #[error("Invalid account_to_close Discriminator")]
    InvalidAccountToCloseDiscriminator,
    #[error("Cannot close account")]
    CannotCloseAccount,
    #[error("Cannot close account - Already closed")]
    CannotCloseAccountAlreadyClosed,
    #[error("Cannot close account - Not enough epochs have passed since consensus reached")]
    CannotCloseAccountNotEnoughEpochs,
    #[error("Cannot close account - No receiver provided")]
    CannotCloseAccountNoReceiverProvided,
    #[error("Cannot close account - No enough accounts")]
    CannotCloseAccountNoEnoughAccounts,
    #[error("Cannot close epoch state account - Epoch state needs all other accounts to be closed first")]
    CannotCloseEpochStateAccount,
    #[error("Invalid NCN Fee wallet")]
    InvalidNCNFeeWallet,
    #[error("Epoch is closing down")]
    EpochIsClosingDown,
    #[error("Marker exists")]
    MarkerExists,
}

impl<T> DecodeError<T> for NCNProgramError {
    fn type_of() -> &'static str {
        "jito::weight_table"
    }
}

impl From<NCNProgramError> for ProgramError {
    fn from(e: NCNProgramError) -> Self {
        Self::Custom(e as u32)
    }
}

impl From<NCNProgramError> for u64 {
    fn from(e: NCNProgramError) -> Self {
        e as Self
    }
}

impl From<NCNProgramError> for u32 {
    fn from(e: NCNProgramError) -> Self {
        e as Self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_codes() {
        // Test base error codes are correct
        assert_eq!(NCNProgramError::DenominatorIsZero as u32, 0x2100);
        assert_eq!(NCNProgramError::IncorrectWeightTableAdmin as u32, 0x2200);

        // Test sequential error codes
        assert_eq!(
            NCNProgramError::ArithmeticOverflow as u32,
            NCNProgramError::DenominatorIsZero as u32 + 1
        );
        assert_eq!(
            NCNProgramError::ArithmeticUnderflowError as u32,
            NCNProgramError::ArithmeticOverflow as u32 + 1
        );
    }

    #[test]
    fn test_error_messages() {
        // Test error messages match their definitions
        assert_eq!(
            NCNProgramError::DenominatorIsZero.to_string(),
            "Zero in the denominator"
        );
        assert_eq!(NCNProgramError::ArithmeticOverflow.to_string(), "Overflow");
        assert_eq!(
            NCNProgramError::WeightTableNotFinalized.to_string(),
            "Weight table not finalized"
        );
        assert_eq!(
            NCNProgramError::InvalidMerkleProof.to_string(),
            "Invalid merkle proof"
        );
    }

    #[test]
    fn test_program_error_conversion() {
        // Test conversion to ProgramError
        let program_error: ProgramError = NCNProgramError::DenominatorIsZero.into();
        assert_eq!(
            program_error,
            ProgramError::Custom(NCNProgramError::DenominatorIsZero as u32)
        );

        let program_error: ProgramError = NCNProgramError::WeightTableNotFinalized.into();
        assert_eq!(
            program_error,
            ProgramError::Custom(NCNProgramError::WeightTableNotFinalized as u32)
        );
    }

    #[test]
    fn test_numeric_conversions() {
        // Test conversion to u64
        let error_u64: u64 = NCNProgramError::DenominatorIsZero.into();
        assert_eq!(error_u64, 0x2100);

        // Test conversion to u32
        let error_u32: u32 = NCNProgramError::DenominatorIsZero.into();
        assert_eq!(error_u32, 0x2100);

        // Test conversion for different error types
        assert_eq!(
            u64::from(NCNProgramError::IncorrectWeightTableAdmin),
            0x2200
        );
        assert_eq!(
            u32::from(NCNProgramError::IncorrectWeightTableAdmin),
            0x2200
        );
    }

    #[test]
    fn test_decode_error_type() {
        assert_eq!(
            <NCNProgramError as DecodeError<ProgramError>>::type_of(),
            "jito::weight_table"
        );
    }

    #[test]
    fn test_error_equality() {
        // Test PartialEq implementation
        assert_eq!(
            NCNProgramError::DenominatorIsZero,
            NCNProgramError::DenominatorIsZero
        );
        assert_ne!(
            NCNProgramError::DenominatorIsZero,
            NCNProgramError::ArithmeticOverflow
        );

        // Test with different error variants
        assert_eq!(
            NCNProgramError::WeightTableNotFinalized,
            NCNProgramError::WeightTableNotFinalized
        );
        assert_ne!(
            NCNProgramError::WeightTableNotFinalized,
            NCNProgramError::InvalidMerkleProof
        );
    }
}
