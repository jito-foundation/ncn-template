use jito_bytemuck::AccountDeserialize;
use jito_jsm_core::loader::{load_signer, load_token_mint};
use jito_restaking_core::ncn::Ncn;
use ncn_program_core::{config::Config, vault_registry::VaultRegistry};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, msg, program_error::ProgramError,
    pubkey::Pubkey,
};

/// Registers a new staked token mint in the vault registry.
///
/// ### Parameters:
/// - `weight`: Optional initial weight for the token
///
/// ### Accounts:
/// 1. `[]` config: NCN configuration account
/// 2. `[writable]` vault_registry: The vault registry to update
/// 3. `[]` ncn: The NCN account
/// 4. `[]` st_mint: The stake token mint to register
/// 5. `[signer]` weight_table_admin: Admin authorized to register tokens
pub fn process_admin_register_st_mint(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    weight: Option<u128>,
) -> ProgramResult {
    let [config, ncn, st_mint, vault_registry, admin] = accounts else {
        msg!("Error: Not enough account keys provided");
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    Config::load(program_id, config, ncn.key, false)?;
    VaultRegistry::load(program_id, vault_registry, ncn.key, true)?;
    Ncn::load(&jito_restaking_program::id(), ncn, false)?;
    load_token_mint(st_mint)?;
    load_signer(admin, false)?;

    {
        let ncn_data = ncn.data.borrow();
        let ncn_account = Ncn::try_from_slice_unchecked(&ncn_data)?;

        if ncn_account.ncn_program_admin.ne(admin.key) {
            msg!("Error: Admin is not the NCN program admin");
            return Err(ProgramError::InvalidAccountData);
        }
    }

    let mut vault_registry_data = vault_registry.data.borrow_mut();
    let vault_registry_account =
        VaultRegistry::try_from_slice_unchecked_mut(&mut vault_registry_data)?;

    let weight = weight.unwrap_or_default();
    msg!("Registering ST mint with weight: {}", weight);

    vault_registry_account.register_st_mint(st_mint.key, weight)?;

    Ok(())
}
