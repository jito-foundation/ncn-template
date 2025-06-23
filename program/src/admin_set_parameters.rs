use jito_bytemuck::{types::PodU64, AccountDeserialize};
use jito_jsm_core::loader::load_signer;
use jito_restaking_core::ncn::Ncn;
use ncn_program_core::{
    config::Config,
    constants::{
        MAX_EPOCHS_AFTER_CONSENSUS_BEFORE_CLOSE, MAX_EPOCHS_BEFORE_STALL,
        MAX_VALID_SLOTS_AFTER_CONSENSUS, MIN_EPOCHS_AFTER_CONSENSUS_BEFORE_CLOSE,
        MIN_EPOCHS_BEFORE_STALL, MIN_VALID_SLOTS_AFTER_CONSENSUS,
    },
    error::NCNProgramError,
};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, msg, program_error::ProgramError,
    pubkey::Pubkey,
};

/// Updates program configuration parameters after initialization.
///
/// ### Parameters:
/// - `starting_valid_epoch`: Optional starting epoch
/// - `epochs_before_stall`: Optional number of epochs before stall
/// - `epochs_after_consensus_before_close`: Optional number of epochs after consensus before close
/// - `valid_slots_after_consensus`: Optional number of valid slots after consensus
///
/// ### Accounts:
/// 1. `[writable]` config: NCN configuration account
/// 2. `[]` ncn: The NCN account
/// 3. `[signer]` ncn_admin: Admin authority for the NCN
pub fn process_admin_set_parameters(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    starting_valid_epoch: Option<u64>,
    epochs_before_stall: Option<u64>,
    epochs_after_consensus_before_close: Option<u64>,
    valid_slots_after_consensus: Option<u64>,
) -> ProgramResult {
    msg!("Starting admin_set_parameters instruction");
    let [config, ncn_account, ncn_admin] = accounts else {
        msg!("Error: Not enough account keys provided");
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    msg!("Verifying admin signer");
    load_signer(ncn_admin, true)?;

    msg!("Loading and verifying accounts");
    Config::load(program_id, config, ncn_account.key, true)?;
    Ncn::load(&jito_restaking_program::id(), ncn_account, false)?;

    {
        msg!("Verifying NCN admin");
        let ncn_data = ncn_account.data.borrow();
        let ncn = Ncn::try_from_slice_unchecked(&ncn_data)?;
        if ncn.admin != *ncn_admin.key {
            msg!("Error: Incorrect NCN admin");
            return Err(NCNProgramError::IncorrectNcnAdmin.into());
        }
    }

    let mut config_data = config.try_borrow_mut_data()?;
    let config = Config::try_from_slice_unchecked_mut(&mut config_data)?;

    msg!("Verifying NCN account");
    if config.ncn != *ncn_account.key {
        msg!("Error: Incorrect NCN account");
        return Err(NCNProgramError::IncorrectNcn.into());
    }

    if let Some(epoch) = starting_valid_epoch {
        msg!(
            "Updating valid_starting_epoch from {} to {}",
            u64::from(config.starting_valid_epoch),
            epoch
        );
        config.starting_valid_epoch = PodU64::from(epoch);
    }

    if let Some(epochs) = epochs_before_stall {
        msg!("Validating epochs_before_stall value: {}", epochs);
        if !(MIN_EPOCHS_BEFORE_STALL..=MAX_EPOCHS_BEFORE_STALL).contains(&epochs) {
            msg!("Error: Invalid epochs_before_stall value");
            return Err(NCNProgramError::InvalidEpochsBeforeStall.into());
        }
        msg!(
            "Updating epochs_before_stall from {} to {}",
            u64::from(config.epochs_before_stall),
            epochs
        );
        config.epochs_before_stall = PodU64::from(epochs);
    }

    if let Some(epochs) = epochs_after_consensus_before_close {
        msg!(
            "Validating epochs_after_consensus_before_close value: {}",
            epochs
        );
        if !(MIN_EPOCHS_AFTER_CONSENSUS_BEFORE_CLOSE..=MAX_EPOCHS_AFTER_CONSENSUS_BEFORE_CLOSE)
            .contains(&epochs)
        {
            msg!("Error: Invalid epochs_after_consensus_before_close value");
            return Err(NCNProgramError::InvalidEpochsBeforeClose.into());
        }
        msg!(
            "Updating epochs_after_consensus_before_close from {} to {}",
            u64::from(config.epochs_after_consensus_before_close),
            epochs
        );
        config.epochs_after_consensus_before_close = PodU64::from(epochs);
    }

    if let Some(slots) = valid_slots_after_consensus {
        msg!("Validating valid_slots_after_consensus value: {}", slots);
        if !(MIN_VALID_SLOTS_AFTER_CONSENSUS..=MAX_VALID_SLOTS_AFTER_CONSENSUS).contains(&slots) {
            msg!("Error: Invalid valid_slots_after_consensus value");
            return Err(NCNProgramError::InvalidSlotsAfterConsensus.into());
        }
        msg!(
            "Updating valid_slots_after_consensus from {} to {}",
            u64::from(config.valid_slots_after_consensus),
            slots
        );
        config.valid_slots_after_consensus = PodU64::from(slots);
    }

    Ok(())
}
