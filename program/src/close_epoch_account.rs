use jito_bytemuck::{AccountDeserialize, Discriminator};
use jito_jsm_core::loader::load_system_program;
use jito_restaking_core::ncn::Ncn;
use ncn_program_core::{
    account_payer::AccountPayer,
    ballot_box::BallotBox,
    config::Config as NcnConfig,
    epoch_marker::EpochMarker,
    epoch_snapshot::{EpochSnapshot, OperatorSnapshot},
    epoch_state::EpochState,
    error::NCNProgramError,
    weight_table::WeightTable,
};
use solana_program::{
    account_info::AccountInfo, clock::Clock, entrypoint::ProgramResult,
    epoch_schedule::EpochSchedule, msg, program_error::ProgramError, pubkey::Pubkey,
    sysvar::Sysvar,
};

/// Crank Closes all accounts associated with an epoch
#[allow(clippy::cognitive_complexity)]
pub fn process_close_epoch_account(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    epoch: u64,
) -> ProgramResult {
    msg!("Processing close epoch account for epoch: {}", epoch);

    let [epoch_marker, epoch_state, config, ncn, account_to_close, account_payer, system_program] =
        accounts
    else {
        msg!("Error: Not enough account keys provided");
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    msg!("Checking system program");
    load_system_program(system_program)?;

    msg!("Loading NCN account: {}", ncn.key);
    Ncn::load(&jito_restaking_program::id(), ncn, false)?;

    msg!("Loading epoch state for NCN: {}, epoch: {}", ncn.key, epoch);
    EpochState::load(program_id, epoch_state, ncn.key, epoch, false)?;

    msg!("Loading NCN config for NCN: {}", ncn.key);
    NcnConfig::load(program_id, config, ncn.key, false)?;

    msg!("Loading account payer: {}", account_payer.key);
    AccountPayer::load(program_id, account_payer, ncn.key, false)?;

    msg!(
        "Checking epoch marker doesn't exist: {}, epoch: {}",
        ncn.key,
        epoch
    );
    EpochMarker::check_dne(program_id, epoch_marker, ncn.key, epoch)?;

    let closing_epoch_state = account_to_close.key.eq(epoch_state.key);
    msg!(
        "Checking if closing epoch state account: {}",
        closing_epoch_state
    );

    // Empty Account Check
    if account_to_close.data_is_empty() {
        msg!("Error: Account already closed: {}", account_to_close.key);
        return Err(NCNProgramError::CannotCloseAccountAlreadyClosed.into());
    }

    {
        msg!("Getting config data");
        let config_data = config.try_borrow_data()?;
        let config_account = NcnConfig::try_from_slice_unchecked(&config_data)?;

        msg!("Getting epoch state data");
        let mut epoch_state_data = epoch_state.try_borrow_mut_data()?;
        let epoch_state_account = EpochState::try_from_slice_unchecked_mut(&mut epoch_state_data)?;

        // Epoch Check - epochs after consensus is reached
        {
            let epochs_after_consensus_before_close =
                config_account.epochs_after_consensus_before_close();
            msg!(
                "Epochs required after consensus before close: {}",
                epochs_after_consensus_before_close
            );

            msg!("Getting current slot and epoch schedule");
            let current_slot = Clock::get()?.slot;
            let epoch_schedule = EpochSchedule::get()?;
            msg!("Current slot: {}", current_slot);

            msg!("Checking if enough epochs have passed since consensus was reached");
            let can_close_epoch_accounts = epoch_state_account.can_close_epoch_accounts(
                &epoch_schedule,
                epochs_after_consensus_before_close,
                current_slot,
            )?;

            if !can_close_epoch_accounts {
                msg!("Error: Not enough epochs have passed since consensus reached");
                return Err(NCNProgramError::CannotCloseAccountNotEnoughEpochs.into());
            }
            msg!("Enough epochs have passed, can close epoch accounts");

            msg!("Setting epoch state as closing");
            epoch_state_account.set_is_closing();
        }

        // Account Check
        {
            msg!("Checking account discriminator");
            let discriminator = {
                if closing_epoch_state {
                    // Cannot borrow the data again
                    msg!("Using EpochState discriminator for closing epoch state");
                    EpochState::DISCRIMINATOR
                } else {
                    msg!("Getting discriminator from account to close");
                    let account_to_close_data = account_to_close.try_borrow_data()?;
                    account_to_close_data[0]
                }
            };
            msg!("Account discriminator: {}", discriminator);

            match discriminator {
                EpochState::DISCRIMINATOR => {
                    msg!("Account is an EpochState, loading to close");
                    EpochState::load_to_close(epoch_state_account, ncn.key, epoch)?;
                    msg!("Closing epoch state");
                    epoch_state_account.close_epoch_state();
                }
                WeightTable::DISCRIMINATOR => {
                    msg!("Account is a WeightTable, loading to close");
                    WeightTable::load_to_close(program_id, account_to_close, ncn.key, epoch)?;
                    msg!("Closing weight table");
                    epoch_state_account.close_weight_table();
                }
                EpochSnapshot::DISCRIMINATOR => {
                    msg!("Account is an EpochSnapshot, loading to close");
                    EpochSnapshot::load_to_close(program_id, account_to_close, ncn.key, epoch)?;
                    msg!("Closing epoch snapshot");
                    epoch_state_account.close_epoch_snapshot();
                }
                OperatorSnapshot::DISCRIMINATOR => {
                    msg!("Account is an OperatorSnapshot, loading to close");
                    OperatorSnapshot::load_to_close(program_id, account_to_close, ncn.key, epoch)?;
                    let account_to_close_data = account_to_close.try_borrow_data()?;
                    let account_to_close_struct =
                        OperatorSnapshot::try_from_slice_unchecked(&account_to_close_data)?;
                    let ncn_operator_index = account_to_close_struct.ncn_operator_index() as usize;
                    msg!(
                        "Closing operator snapshot with index: {}",
                        ncn_operator_index
                    );
                    epoch_state_account.close_operator_snapshot(ncn_operator_index);
                }
                BallotBox::DISCRIMINATOR => {
                    msg!("Account is a BallotBox, loading to close");
                    BallotBox::load_to_close(program_id, account_to_close, ncn.key, epoch)?;
                    msg!("Closing ballot box");
                    epoch_state_account.close_ballot_box();
                }
                _ => {
                    msg!("Error: Invalid account discriminator: {}", discriminator);
                    return Err(NCNProgramError::InvalidAccountToCloseDiscriminator.into());
                }
            }
            msg!("Account closed successfully in epoch state tracking");
        }
    }

    if closing_epoch_state {
        msg!("Closing epoch state, creating epoch marker");

        msg!("Finding program address for epoch marker");
        let (epoch_marker_pda, epoch_marker_bump, mut epoch_marker_seeds) =
            EpochMarker::find_program_address(program_id, ncn.key, epoch);
        epoch_marker_seeds.push(vec![epoch_marker_bump]);

        msg!(
            "Generated epoch marker PDA: {}, bump: {}",
            epoch_marker_pda,
            epoch_marker_bump
        );

        if epoch_marker_pda != *epoch_marker.key {
            msg!(
                "Error: Invalid epoch marker PDA. Expected: {}, got: {}",
                epoch_marker_pda,
                epoch_marker.key
            );
            return Err(ProgramError::InvalidSeeds);
        }

        msg!(
            "Creating epoch marker account with {} bytes",
            EpochMarker::SIZE
        );
        AccountPayer::pay_and_create_account(
            program_id,
            ncn.key,
            account_payer,
            epoch_marker,
            system_program,
            program_id,
            EpochMarker::SIZE,
            &epoch_marker_seeds,
        )?;
        msg!(
            "Epoch marker account created successfully: {}",
            epoch_marker.key
        );

        msg!("Initializing epoch marker account with discriminator");
        let mut epoch_marker_data = epoch_marker.try_borrow_mut_data()?;
        epoch_marker_data[0] = EpochMarker::DISCRIMINATOR;
        let epoch_marker = EpochMarker::try_from_slice_unchecked_mut(&mut epoch_marker_data)?;

        msg!("Getting current slot for epoch marker");
        let slot_closed = Clock::get()?.slot;
        msg!("Current slot: {}", slot_closed);

        msg!(
            "Creating new epoch marker for NCN: {}, epoch: {}, slot: {}",
            ncn.key,
            epoch,
            slot_closed
        );
        *epoch_marker = EpochMarker::new(ncn.key, epoch, slot_closed);
        msg!("Epoch marker initialized successfully");
    }

    msg!("Closing account: {}", account_to_close.key);
    AccountPayer::close_account(program_id, account_payer, account_to_close)?;
    msg!(
        "Account closed successfully, lamports transferred to account payer: {}",
        account_payer.key
    );

    msg!(
        "Close epoch account completed successfully for epoch: {}",
        epoch
    );
    Ok(())
}
