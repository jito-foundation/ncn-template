use jito_bytemuck::AccountDeserialize;
use jito_jsm_core::loader::load_signer;
use jito_restaking_core::ncn::Ncn;
use ncn_program_core::{
    epoch_state::EpochState, error::NCNProgramError, weight_table::WeightTable,
};
use solana_program::{
    account_info::AccountInfo, clock::Clock, entrypoint::ProgramResult, msg,
    program_error::ProgramError, pubkey::Pubkey, sysvar::Sysvar,
};

/// Updates weight table
pub fn process_admin_set_weight(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    st_mint: &Pubkey,
    epoch: u64,
    weight: u128,
) -> ProgramResult {
    msg!(
        "Processing admin set weight for st_mint: {}, epoch: {}, weight: {}",
        st_mint,
        epoch,
        weight
    );

    let [epoch_state, ncn, weight_table, weight_table_admin] = accounts else {
        msg!("Error: Not enough account keys provided");
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    msg!("Loading NCN account: {}", ncn.key);
    Ncn::load(&jito_restaking_program::id(), ncn, false)?;

    msg!("Getting weight table admin from NCN account");
    let ncn_weight_table_admin = {
        let ncn_data = ncn.data.borrow();
        let ncn = Ncn::try_from_slice_unchecked(&ncn_data)?;
        ncn.weight_table_admin
    };
    msg!("NCN weight table admin: {}", ncn_weight_table_admin);

    msg!("Verifying weight table admin is the signer");
    load_signer(weight_table_admin, true)?;

    msg!("Loading epoch state for NCN: {}, epoch: {}", ncn.key, epoch);
    EpochState::load(program_id, epoch_state, ncn.key, epoch, true)?;

    msg!(
        "Loading weight table for NCN: {}, epoch: {}",
        ncn.key,
        epoch
    );
    WeightTable::load(program_id, weight_table, ncn.key, epoch, true)?;

    if ncn_weight_table_admin.ne(weight_table_admin.key) {
        msg!(
            "Error: Incorrect weight table admin. Expected: {}, got: {}",
            ncn_weight_table_admin,
            weight_table_admin.key
        );
        return Err(NCNProgramError::IncorrectWeightTableAdmin.into());
    }

    msg!("Preparing to modify weight table");
    let mut weight_table_data = weight_table.try_borrow_mut_data()?;
    let weight_table_account = WeightTable::try_from_slice_unchecked_mut(&mut weight_table_data)?;

    msg!("Checking if weight table is initialized");
    weight_table_account.check_table_initialized()?;

    if weight_table_account.finalized() {
        msg!("Error: Weight table is already finalized");
        return Err(ProgramError::InvalidAccountData);
    }

    msg!("Getting current slot");
    let current_slot = Clock::get()?.slot;
    msg!("Current slot: {}", current_slot);

    msg!(
        "Setting weight for st_mint: {}, weight: {}",
        st_mint,
        weight
    );
    weight_table_account.set_weight(st_mint, weight, current_slot)?;
    msg!("Weight set successfully");

    // Update Epoch State
    msg!("Updating epoch state account");
    {
        let mut epoch_state_data = epoch_state.try_borrow_mut_data()?;
        let epoch_state_account = EpochState::try_from_slice_unchecked_mut(&mut epoch_state_data)?;

        let weight_count = weight_table_account.weight_count() as u64;
        let st_mint_count = weight_table_account.st_mint_count() as u64;

        msg!(
            "Updating epoch state with weight count: {}, st_mint count: {}",
            weight_count,
            st_mint_count
        );

        epoch_state_account.update_set_weight(weight_count, st_mint_count);
    }
    msg!("Epoch state updated successfully");

    msg!(
        "Admin set weight completed successfully for st_mint: {}, epoch: {}, weight: {}",
        st_mint,
        epoch,
        weight
    );

    Ok(())
}
