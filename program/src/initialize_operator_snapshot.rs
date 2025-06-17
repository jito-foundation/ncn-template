use jito_bytemuck::{AccountDeserialize, Discriminator};
use jito_jsm_core::loader::{load_system_account, load_system_program};
use jito_restaking_core::{ncn::Ncn, ncn_operator_state::NcnOperatorState, operator::Operator};
use ncn_program_core::{
    account_payer::AccountPayer,
    config::Config,
    epoch_marker::EpochMarker,
    epoch_snapshot::{EpochSnapshot, OperatorSnapshot},
    epoch_state::EpochState,
    error::NCNProgramError,
    loaders::load_ncn_epoch,
    stake_weight::StakeWeights,
};
use solana_program::{
    account_info::AccountInfo, clock::Clock, entrypoint::ProgramResult, msg,
    program_error::ProgramError, pubkey::Pubkey, sysvar::Sysvar,
};

/// Initializes a snapshot for a specific operator, storing their stake weights.
///
/// ### Parameters:
/// - `epoch`: The target epoch
///
/// ### Accounts:
/// 1. `[]` epoch_marker: Marker account to prevent duplicate initialization
/// 2. `[writable]` epoch_state: The epoch state account for the target epoch
/// 3. `[]` config: NCN configuration account
/// 4. `[]` ncn: The NCN account
/// 5. `[]` operator: The operator account to snapshot
/// 6. `[]` ncn_operator_ticket: The connection between NCN and operator
/// 7. `[writable]` operator_snapshot: Operator snapshot account to initialize
/// 8. `[writable, signer]` account_payer: Account paying for initialization
/// 9. `[]` system_program: Solana System Program
pub fn process_initialize_operator_snapshot(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    epoch: u64,
) -> ProgramResult {
    msg!(
        "Processing initialize operator snapshot for epoch: {}",
        epoch
    );

    let [epoch_marker, epoch_state, config, restaking_config, ncn, operator, ncn_operator_state, epoch_snapshot, operator_snapshot, account_payer, system_program] =
        accounts
    else {
        msg!("Error: Not enough account keys provided");
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    msg!("Loading and checking Accounts",);
    EpochState::load_and_check_is_closing(program_id, epoch_state, ncn.key, epoch, false)?;
    Config::load(program_id, config, ncn.key, false)?;
    Ncn::load(&jito_restaking_program::id(), ncn, false)?;
    Operator::load(&jito_restaking_program::id(), operator, false)?;
    NcnOperatorState::load(
        &jito_restaking_program::id(),
        ncn_operator_state,
        ncn,
        operator,
        false,
    )?;
    EpochSnapshot::load(program_id, epoch_snapshot, ncn.key, epoch, false)?;
    load_system_account(operator_snapshot, true)?;
    load_system_program(system_program)?;
    AccountPayer::load(program_id, account_payer, ncn.key, true)?;
    EpochMarker::check_dne(program_id, epoch_marker, ncn.key, epoch)?;

    msg!("Finding program address for operator snapshot");
    let (operator_snapshot_pubkey, operator_snapshot_bump, mut operator_snapshot_seeds) =
        OperatorSnapshot::find_program_address(program_id, operator.key, ncn.key, epoch);
    operator_snapshot_seeds.push(vec![operator_snapshot_bump]);

    msg!(
        "Generated operator snapshot PDA: {}, bump: {}",
        operator_snapshot_pubkey,
        operator_snapshot_bump
    );

    if operator_snapshot_pubkey.ne(operator_snapshot.key) {
        msg!(
            "Error: Operator snapshot account is not at the correct PDA. Expected: {}, got: {}",
            operator_snapshot_pubkey,
            operator_snapshot.key
        );
        return Err(ProgramError::InvalidAccountData);
    }

    // Cannot create Operator snapshot if the operator index is greater than the operator count
    {
        msg!("Checking operator index is within bounds");
        let epoch_snapshot_data = epoch_snapshot.data.borrow();
        let epoch_snapshot = EpochSnapshot::try_from_slice_unchecked(&epoch_snapshot_data)?;

        let ncn_operator_state_data = ncn_operator_state.data.borrow();
        let ncn_operator_state =
            NcnOperatorState::try_from_slice_unchecked(&ncn_operator_state_data)?;

        let operator_count = epoch_snapshot.operator_count();
        let operator_index = ncn_operator_state.index();

        msg!(
            "Operator index: {}, operator count: {}",
            operator_index,
            operator_count
        );

        if operator_index >= operator_count {
            msg!(
                "Error: Operator index is out of bounds. Index: {}, Count: {}",
                operator_index,
                operator_count
            );
            return Err(NCNProgramError::OperatorIsNotInSnapshot.into());
        }
    }

    msg!(
        "Initializing Operator snapshot {} for NCN: {} at epoch: {}",
        epoch_snapshot.key,
        ncn.key,
        epoch
    );
    msg!(
        "Creating operator snapshot account with {} bytes",
        OperatorSnapshot::SIZE
    );
    AccountPayer::pay_and_create_account(
        program_id,
        ncn.key,
        account_payer,
        operator_snapshot,
        system_program,
        program_id,
        OperatorSnapshot::SIZE,
        &operator_snapshot_seeds,
    )?;
    msg!(
        "Operator snapshot account created successfully: {}",
        operator_snapshot.key
    );

    msg!("Getting current slot");
    let current_slot = Clock::get()?.slot;
    msg!("Current slot: {}", current_slot);

    msg!("Loading NCN epoch length from restaking config");
    let (_, ncn_epoch_length) = load_ncn_epoch(restaking_config, current_slot, None)?;
    msg!("NCN epoch length: {}", ncn_epoch_length);

    msg!("Determining operator activity status");
    let (is_active, ncn_operator_index): (bool, u64) = {
        let ncn_operator_state_data = ncn_operator_state.data.borrow();
        let ncn_operator_state_account =
            NcnOperatorState::try_from_slice_unchecked(&ncn_operator_state_data)?;

        // If the NCN removes an operator, it should immediately be barred from the snapshot
        let ncn_operator_okay = ncn_operator_state_account
            .ncn_opt_in_state
            .is_active(current_slot, ncn_epoch_length)?;
        msg!("NCN operator opt-in state active: {}", ncn_operator_okay);

        // If the operator removes itself from the ncn, it should still be able to participate
        // while it is cooling down
        let operator_ncn_okay = ncn_operator_state_account
            .operator_opt_in_state
            .is_active_or_cooldown(current_slot, ncn_epoch_length)?;

        let ncn_operator_index = ncn_operator_state_account.index();
        msg!("NCN operator index: {}", ncn_operator_index);

        (ncn_operator_okay && operator_ncn_okay, ncn_operator_index)
    };
    msg!("Operator is active: {}", is_active);

    msg!("Getting vault count from epoch snapshot");
    let vault_count = {
        let epoch_snapshot_data = epoch_snapshot.data.borrow();
        let epoch_snapshot_account = EpochSnapshot::try_from_slice_unchecked(&epoch_snapshot_data)?;
        epoch_snapshot_account.vault_count()
    };
    msg!("Vault count: {}", vault_count);

    msg!("Getting operator fee and index");
    let (operator_fee_bps, operator_index): (u16, u64) = {
        let operator_data = operator.data.borrow();
        let operator_account = Operator::try_from_slice_unchecked(&operator_data)?;
        (
            operator_account.operator_fee_bps.into(),
            operator_account.index(),
        )
    };
    msg!(
        "Operator fee (bps): {}, operator index: {}",
        operator_fee_bps,
        operator_index
    );

    msg!("Initializing operator snapshot account with discriminator");
    let mut operator_snapshot_data = operator_snapshot.try_borrow_mut_data()?;
    operator_snapshot_data[0] = OperatorSnapshot::DISCRIMINATOR;
    let operator_snapshot_account =
        OperatorSnapshot::try_from_slice_unchecked_mut(&mut operator_snapshot_data)?;

    msg!("Setting operator snapshot fields");
    operator_snapshot_account.initialize(
        operator.key,
        ncn.key,
        epoch,
        operator_snapshot_bump,
        current_slot,
        is_active,
        ncn_operator_index,
        operator_index,
        operator_fee_bps,
        vault_count,
    )?;
    msg!("Operator snapshot initialized successfully");

    // Increment operator registration for an inactive operator
    if !is_active {
        msg!("Operator is inactive, incrementing operator registration in epoch snapshot");
        let mut epoch_snapshot_data = epoch_snapshot.try_borrow_mut_data()?;
        let epoch_snapshot_account =
            EpochSnapshot::try_from_slice_unchecked_mut(&mut epoch_snapshot_data)?;

        epoch_snapshot_account.increment_operator_registration(
            current_slot,
            0,
            &StakeWeights::default(),
        )?;
        msg!("Epoch snapshot operator registration incremented successfully");
    }

    // Update Epoch State
    msg!("Updating epoch state account");
    {
        let mut epoch_state_data = epoch_state.try_borrow_mut_data()?;
        let epoch_state_account = EpochState::try_from_slice_unchecked_mut(&mut epoch_state_data)?;
        epoch_state_account
            .update_realloc_operator_snapshot(ncn_operator_index as usize, is_active)?;
    }
    msg!("Epoch state updated successfully");

    msg!(
        "Initialize operator snapshot completed successfully for operator: {}, NCN: {}, epoch: {}",
        operator.key,
        ncn.key,
        epoch
    );

    Ok(())
}
