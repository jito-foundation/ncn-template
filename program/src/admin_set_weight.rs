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

/// Admin instruction to set the weight for a specific staked token mint in a given epoch's WeightTable.
///
/// ### Parameters:
/// - `st_mint`: Pubkey of the staked token mint.
/// - `weight`: Weight value (u128) for the token.
/// - `epoch`: Target epoch.
///
/// ### Accounts:
/// 1. `[writable]` epoch_state: Epoch state for the target epoch.
/// 2. `[]` ncn: The NCN account.
/// 3. `[writable]` weight_table: The weight table to update.
/// 4. `[signer]` weight_table_admin: Admin authorized to update weights.
pub fn process_admin_set_weight(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    st_mint: &Pubkey,
    epoch: u64,
    weight: u128,
) -> ProgramResult {
    let [epoch_state, ncn, weight_table, weight_table_admin] = accounts else {
        msg!("Error: Not enough account keys provided");
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    Ncn::load(&jito_restaking_program::id(), ncn, false)?;

    let ncn_weight_table_admin = {
        let ncn_data = ncn.data.borrow();
        let ncn = Ncn::try_from_slice_unchecked(&ncn_data)?;
        ncn.weight_table_admin
    };
    msg!("NCN weight table admin: {}", ncn_weight_table_admin);

    load_signer(weight_table_admin, true)?;
    EpochState::load(program_id, epoch_state, ncn.key, epoch, true)?;
    WeightTable::load(program_id, weight_table, ncn.key, epoch, true)?;

    if ncn_weight_table_admin.ne(weight_table_admin.key) {
        msg!(
            "Error: Incorrect weight table admin. Expected: {}, got: {}",
            ncn_weight_table_admin,
            weight_table_admin.key
        );
        return Err(NCNProgramError::IncorrectWeightTableAdmin.into());
    }

    let mut weight_table_data = weight_table.try_borrow_mut_data()?;
    let weight_table_account = WeightTable::try_from_slice_unchecked_mut(&mut weight_table_data)?;

    weight_table_account.check_table_initialized()?;

    if weight_table_account.finalized() {
        msg!("Error: Weight table is already finalized");
        return Err(ProgramError::InvalidAccountData);
    }

    let current_slot = Clock::get()?.slot;

    msg!(
        "Setting weight for st_mint: {}, weight: {}",
        st_mint,
        weight
    );
    weight_table_account.set_weight(st_mint, weight, current_slot)?;

    // Update Epoch State
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

    Ok(())
}
