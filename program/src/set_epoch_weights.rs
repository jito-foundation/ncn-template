use jito_bytemuck::AccountDeserialize;
use jito_restaking_core::ncn::Ncn;
use ncn_program_core::{
    epoch_state::EpochState, error::NCNProgramError, vault_registry::VaultRegistry,
    weight_table::WeightTable,
};
use solana_program::{
    account_info::AccountInfo, clock::Clock, entrypoint::ProgramResult, msg,
    program_error::ProgramError, pubkey::Pubkey, sysvar::Sysvar,
};

/// Sets weight in the weight_table using weights from vault_registry
pub fn process_set_epoch_weights(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    epoch: u64,
) -> ProgramResult {
    msg!("Starting set_epoch_weights instruction for epoch {}", epoch);

    let [epoch_state, ncn, vault_registry, weight_table] = accounts else {
        msg!("Error: Not enough account keys provided");
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    msg!("Loading required accounts...");
    EpochState::load(program_id, epoch_state, ncn.key, epoch, true)?;
    Ncn::load(&jito_restaking_program::id(), ncn, false)?;
    WeightTable::load(program_id, weight_table, ncn.key, epoch, true)?;
    VaultRegistry::load(program_id, vault_registry, ncn.key, false)?;
    msg!("All required accounts loaded successfully");

    let mut weight_table_data = weight_table.try_borrow_mut_data()?;
    let weight_table_account = WeightTable::try_from_slice_unchecked_mut(&mut weight_table_data)?;
    msg!("Checking if weight table is initialized...");
    weight_table_account.check_table_initialized()?;
    msg!("Weight table is initialized");

    if weight_table_account.finalized() {
        msg!("Error: Weight table is already finalized");
        return Err(ProgramError::InvalidAccountData);
    }

    msg!("Processing mint entries from vault registry...");
    let mut vault_registry_data = vault_registry.data.borrow_mut();
    let vault_registry_account =
        VaultRegistry::try_from_slice_unchecked_mut(&mut vault_registry_data)?;

    for mint_entry in vault_registry_account.get_valid_mint_entries() {
        let weight_from_mint_entry = mint_entry.weight();
        if weight_from_mint_entry == 0 {
            msg!("Error: Weight is not set for mint entry");
            return Err(NCNProgramError::WeightNotSet.into());
        }

        msg!(
            "Setting weight {} for mint {}",
            weight_from_mint_entry,
            mint_entry.st_mint()
        );
        weight_table_account.set_weight(
            mint_entry.st_mint(),
            weight_from_mint_entry,
            Clock::get()?.slot,
        )?;
    }
    msg!("All mint entries processed successfully");

    // Update Epoch State
    {
        msg!("Updating epoch state...");
        let mut epoch_state_data = epoch_state.try_borrow_mut_data()?;
        let epoch_state_account = EpochState::try_from_slice_unchecked_mut(&mut epoch_state_data)?;
        epoch_state_account.update_set_weight(
            weight_table_account.weight_count() as u64,
            weight_table_account.st_mint_count() as u64,
        );
        msg!(
            "Epoch state updated with weight count: {} and st mint count: {}",
            weight_table_account.weight_count(),
            weight_table_account.st_mint_count()
        );
    }

    msg!("set_epoch_weights instruction completed successfully");
    Ok(())
}
