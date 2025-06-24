use jito_jsm_core::loader::{load_system_account, load_system_program};
use jito_restaking_core::ncn::Ncn;
use ncn_program_core::{
    account_payer::AccountPayer, config::Config as NcnConfig, constants::MAX_REALLOC_BYTES,
    vault_registry::VaultRegistry,
};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, msg, program_error::ProgramError,
    pubkey::Pubkey,
};

/// Initializes the vault registry for tracking vaults and supported stake token mints.
///
/// ### Accounts:
/// 1. `[]` config: NCN configuration account
/// 2. `[writable]` vault_registry: The vault registry account to initialize
/// 3. `[]` ncn: The NCN account
/// 4. `[writable, signer]` account_payer: Account paying for the initialization
/// 5. `[]` system_program: Solana System Program
pub fn process_initialize_vault_registry(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let [ncn_config, vault_registry, ncn, account_payer, system_program] = accounts else {
        msg!("Error: Not enough account keys provided");
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    load_system_account(vault_registry, true)?;
    load_system_program(system_program)?;
    Ncn::load(&jito_restaking_program::id(), ncn, false)?;
    NcnConfig::load(program_id, ncn_config, ncn.key, false)?;
    AccountPayer::load(program_id, account_payer, ncn.key, true)?;

    let (vault_registry_pda, vault_registry_bump, mut vault_registry_seeds) =
        VaultRegistry::find_program_address(program_id, ncn.key);
    vault_registry_seeds.push(vec![vault_registry_bump]);

    if vault_registry_pda != *vault_registry.key {
        msg!("Error: Invalid vault registry PDA");
        return Err(ProgramError::InvalidSeeds);
    }

    AccountPayer::pay_and_create_account(
        program_id,
        ncn.key,
        account_payer,
        vault_registry,
        system_program,
        program_id,
        MAX_REALLOC_BYTES as usize,
        &vault_registry_seeds,
    )?;

    Ok(())
}
