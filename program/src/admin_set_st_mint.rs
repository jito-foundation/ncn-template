use jito_bytemuck::AccountDeserialize;
use jito_jsm_core::loader::load_signer;
use jito_restaking_core::ncn::Ncn;
use ncn_program_core::{config::Config, vault_registry::VaultRegistry};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, msg, program_error::ProgramError,
    pubkey::Pubkey,
};

/// Updates an existing staked token mint in the vault registry.
///
/// ### Parameters:
/// - `st_mint`: Public key of the staked token mint
/// - `weight`: Optional new weight for the token
///
/// ### Accounts:
/// 1. `[]` config: NCN configuration account
/// 2. `[writable]` vault_registry: The vault registry to update
/// 3. `[]` ncn: The NCN account
/// 4. `[signer]` weight_table_admin: Admin authorized to update token weights
pub fn process_admin_set_st_mint(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    st_mint: &Pubkey,
    weight: Option<u128>,
) -> ProgramResult {
    msg!("Starting admin_set_st_mint instruction");
    let [config, ncn, vault_registry, admin] = accounts else {
        msg!("Error: Not enough account keys provided");
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    msg!("Loading and verifying accounts");
    Config::load(program_id, config, ncn.key, false)?;
    VaultRegistry::load(program_id, vault_registry, ncn.key, true)?;
    Ncn::load(&jito_restaking_program::id(), ncn, false)?;

    msg!("Verifying admin signer");
    load_signer(admin, false)?;

    {
        msg!("Verifying NCN program admin");
        let ncn_data = ncn.data.borrow();
        let ncn_account = Ncn::try_from_slice_unchecked(&ncn_data)?;

        if ncn_account.ncn_program_admin.ne(admin.key) {
            msg!("Error: Admin is not the NCN program admin");
            return Err(ProgramError::InvalidAccountData);
        }
    }

    msg!("Updating ST mint in vault registry");
    let mut vault_registry_data = vault_registry.data.borrow_mut();
    let vault_registry_account =
        VaultRegistry::try_from_slice_unchecked_mut(&mut vault_registry_data)?;

    msg!("Setting ST mint to {:?} with weight {:?}", st_mint, weight);
    vault_registry_account.set_st_mint(st_mint, weight)?;
    msg!("Successfully updated ST mint in vault registry");

    msg!("Successfully completed admin_set_st_mint instruction");
    Ok(())
}
