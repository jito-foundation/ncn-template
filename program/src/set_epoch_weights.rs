use jito_bytemuck::AccountDeserialize;
use jito_restaking_core::ncn::Ncn;
use jito_tip_router_core::{
    epoch_state::EpochState, error::TipRouterError, vault_registry::VaultRegistry,
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
    let [epoch_state, ncn, vault_registry, weight_table] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    EpochState::load(program_id, epoch_state, ncn.key, epoch, true)?;
    Ncn::load(&jito_restaking_program::id(), ncn, false)?;
    WeightTable::load(program_id, weight_table, ncn.key, epoch, true)?;
    VaultRegistry::load(program_id, vault_registry, ncn.key, false)?;

    let mut weight_table_data = weight_table.try_borrow_mut_data()?;
    let weight_table_account = WeightTable::try_from_slice_unchecked_mut(&mut weight_table_data)?;
    weight_table_account.check_table_initialized()?;

    if weight_table_account.finalized() {
        msg!("Weight table is finalized");
        return Err(ProgramError::InvalidAccountData);
    }

    let mut vault_registry_data = vault_registry.data.borrow_mut();
    let vault_registry_account =
        VaultRegistry::try_from_slice_unchecked_mut(&mut vault_registry_data)?;

    for mint_entry in vault_registry_account.get_valid_mint_entries() {
        let weight_from_mint_entry = mint_entry.weight();
        if weight_from_mint_entry == 0 {
            msg!("Weight is not set");
            return Err(TipRouterError::WeightNotSet.into());
        }

        weight_table_account.set_weight(
            mint_entry.st_mint(),
            weight_from_mint_entry,
            Clock::get()?.slot,
        )?;
    }

    // Update Epoch State
    {
        let mut epoch_state_data = epoch_state.try_borrow_mut_data()?;
        let epoch_state_account = EpochState::try_from_slice_unchecked_mut(&mut epoch_state_data)?;
        epoch_state_account.update_set_weight(
            weight_table_account.weight_count() as u64,
            weight_table_account.st_mint_count() as u64,
        );
    }

    Ok(())
}
