use jito_bytemuck::{AccountDeserialize, Discriminator};
use jito_jsm_core::loader::{load_signer, load_system_account, load_system_program};
use jito_restaking_core::ncn::Ncn;
use ncn_program_core::{
    account_payer::AccountPayer,
    config::Config,
    constants::{
        MAX_EPOCHS_AFTER_CONSENSUS_BEFORE_CLOSE, MAX_EPOCHS_BEFORE_STALL,
        MAX_VALID_SLOTS_AFTER_CONSENSUS, MIN_EPOCHS_AFTER_CONSENSUS_BEFORE_CLOSE,
        MIN_EPOCHS_BEFORE_STALL, MIN_VALID_SLOTS_AFTER_CONSENSUS,
    },
    error::NCNProgramError,
};
use solana_program::{
    account_info::AccountInfo, clock::Clock, entrypoint::ProgramResult, msg,
    program_error::ProgramError, pubkey::Pubkey, sysvar::Sysvar,
};

/// Initializes the program configuration with parameters for the consensus mechanism. Requires NCN admin signature.
///
/// ### Parameters:
/// - `epochs_before_stall`: Number of epochs before voting is considered stalled
/// - `epochs_after_consensus_before_close`: Number of epochs after consensus before accounts can be closed
/// - `valid_slots_after_consensus`: Number of slots after consensus where voting is still valid
///
/// ### Accounts:
/// 1. `[writable]` config: The config account PDA to initialize `[seeds = [b"config", ncn.key().as_ref()], bump]`
/// 2. `[]` ncn: The NCN account this config belongs to
/// 3. `[signer]` ncn_admin: Admin authority for the NCN
/// 4. `[]` tie_breaker_admin: Pubkey of the admin authorized to break voting ties
/// 5. `[writable, signer]` account_payer: Account paying for the initialization and rent
/// 6. `[]` system_program: Solana System Program
#[allow(clippy::too_many_arguments)]
pub fn process_admin_initialize_config(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    epochs_before_stall: u64,
    epochs_after_consensus_before_close: u64,
    valid_slots_after_consensus: u64,
) -> ProgramResult {
    msg!("Processing admin initialize config with epochs_before_stall: {}, epochs_after_consensus_before_close: {}, valid_slots_after_consensus: {}", 
        epochs_before_stall, epochs_after_consensus_before_close, valid_slots_after_consensus);

    let [config, ncn, ncn_admin, tie_breaker_admin, account_payer, system_program] = accounts
    else {
        msg!("Error: Not enough account keys provided");
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    msg!("Checking config is a system account");
    load_system_account(config, true)?;

    msg!("Checking system program");
    load_system_program(system_program)?;

    msg!("Verifying NCN admin is the signer");
    load_signer(ncn_admin, false)?;

    msg!("Loading NCN account: {}", ncn.key);
    Ncn::load(&jito_restaking_program::id(), ncn, false)?;

    msg!("Loading account payer: {}", account_payer.key);
    AccountPayer::load(program_id, account_payer, ncn.key, true)?;

    msg!("Getting current epoch");
    let epoch = Clock::get()?.epoch;
    msg!("Current epoch: {}", epoch);

    msg!(
        "Validating epochs_before_stall: {} (min: {}, max: {})",
        epochs_before_stall,
        MIN_EPOCHS_BEFORE_STALL,
        MAX_EPOCHS_BEFORE_STALL
    );
    if !(MIN_EPOCHS_BEFORE_STALL..=MAX_EPOCHS_BEFORE_STALL).contains(&epochs_before_stall) {
        msg!(
            "Error: Invalid epochs_before_stall value: {}",
            epochs_before_stall
        );
        return Err(NCNProgramError::InvalidEpochsBeforeStall.into());
    }

    msg!(
        "Validating epochs_after_consensus_before_close: {} (min: {}, max: {})",
        epochs_after_consensus_before_close,
        MIN_EPOCHS_AFTER_CONSENSUS_BEFORE_CLOSE,
        MAX_EPOCHS_AFTER_CONSENSUS_BEFORE_CLOSE
    );
    if !(MIN_EPOCHS_AFTER_CONSENSUS_BEFORE_CLOSE..=MAX_EPOCHS_AFTER_CONSENSUS_BEFORE_CLOSE)
        .contains(&epochs_after_consensus_before_close)
    {
        msg!(
            "Error: Invalid epochs_after_consensus_before_close value: {}",
            epochs_after_consensus_before_close
        );
        return Err(NCNProgramError::InvalidEpochsBeforeClose.into());
    }

    msg!(
        "Validating valid_slots_after_consensus: {} (min: {}, max: {})",
        valid_slots_after_consensus,
        MIN_VALID_SLOTS_AFTER_CONSENSUS,
        MAX_VALID_SLOTS_AFTER_CONSENSUS
    );
    if !(MIN_VALID_SLOTS_AFTER_CONSENSUS..=MAX_VALID_SLOTS_AFTER_CONSENSUS)
        .contains(&valid_slots_after_consensus)
    {
        msg!(
            "Error: Invalid valid_slots_after_consensus value: {}",
            valid_slots_after_consensus
        );
        return Err(NCNProgramError::InvalidSlotsAfterConsensus.into());
    }

    msg!("Verifying NCN admin matches the signer");
    let ncn_data = ncn.data.borrow();
    let ncn_account = Ncn::try_from_slice_unchecked(&ncn_data)?;
    if ncn_account.admin != *ncn_admin.key {
        msg!(
            "Error: Incorrect NCN admin. Expected: {}, got: {}",
            ncn_account.admin,
            ncn_admin.key
        );
        return Err(NCNProgramError::IncorrectNcnAdmin.into());
    }

    msg!("Finding program address for config");
    let (config_pda, config_bump, mut config_seeds) =
        Config::find_program_address(program_id, ncn.key);
    config_seeds.push(vec![config_bump]);

    msg!(
        "Generated config PDA: {}, bump: {}",
        config_pda,
        config_bump
    );

    if config_pda != *config.key {
        msg!(
            "Error: Invalid config PDA. Expected: {}, got: {}",
            config_pda,
            config.key
        );
        return Err(ProgramError::InvalidSeeds);
    }

    msg!("Creating config account with {} bytes", Config::SIZE);
    AccountPayer::pay_and_create_account(
        program_id,
        ncn.key,
        account_payer,
        config,
        system_program,
        program_id,
        Config::SIZE,
        &config_seeds,
    )?;
    msg!("Config account created successfully: {}", config.key);

    msg!("Initializing config account with discriminator");
    let mut config_data = config.try_borrow_mut_data()?;
    config_data[0] = Config::DISCRIMINATOR;
    let config = Config::try_from_slice_unchecked_mut(&mut config_data)?;

    let starting_valid_epoch = epoch;
    msg!(
        "Setting starting_valid_epoch to current epoch: {}",
        starting_valid_epoch
    );

    msg!(
        "Creating new config with tie_breaker_admin: {}",
        tie_breaker_admin.key
    );
    *config = Config::new(
        ncn.key,
        tie_breaker_admin.key,
        starting_valid_epoch,
        valid_slots_after_consensus,
        epochs_before_stall,
        epochs_after_consensus_before_close,
        config_bump,
    );
    msg!("Config initialized successfully");

    msg!("Admin initialize config completed successfully");
    Ok(())
}
