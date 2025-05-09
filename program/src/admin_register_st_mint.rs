use jito_bytemuck::AccountDeserialize;
use jito_jsm_core::loader::{load_signer, load_token_mint};
use jito_restaking_core::ncn::Ncn;
use ncn_program_core::{config::Config, vault_registry::VaultRegistry};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, msg, program_error::ProgramError,
    pubkey::Pubkey,
};

pub fn process_admin_register_st_mint(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    weight: Option<u128>,
) -> ProgramResult {
    msg!("Starting admin_register_st_mint instruction");

    let [config, ncn, st_mint, vault_registry, admin] = accounts else {
        msg!("Error: Not enough account keys provided");
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    msg!("Loading Config account");
    Config::load(program_id, config, ncn.key, false)?;

    msg!("Loading VaultRegistry account");
    VaultRegistry::load(program_id, vault_registry, ncn.key, true)?;

    msg!("Loading NCN account");
    Ncn::load(&jito_restaking_program::id(), ncn, false)?;

    msg!("Loading ST mint account");
    load_token_mint(st_mint)?;

    msg!("Verifying admin signature");
    load_signer(admin, false)?;

    {
        msg!("Checking admin authorization");
        let ncn_data = ncn.data.borrow();
        let ncn_account = Ncn::try_from_slice_unchecked(&ncn_data)?;

        if ncn_account.ncn_program_admin.ne(admin.key) {
            msg!("Error: Admin is not the NCN program admin");
            return Err(ProgramError::InvalidAccountData);
        }
        msg!("Admin authorization verified");
    }

    msg!("Updating vault registry with ST mint");
    let mut vault_registry_data = vault_registry.data.borrow_mut();
    let vault_registry_account =
        VaultRegistry::try_from_slice_unchecked_mut(&mut vault_registry_data)?;

    let weight = weight.unwrap_or_default();
    msg!("Registering ST mint with weight: {}", weight);

    vault_registry_account.register_st_mint(st_mint.key, weight)?;
    msg!("Successfully registered ST mint in vault registry");

    Ok(())
}
