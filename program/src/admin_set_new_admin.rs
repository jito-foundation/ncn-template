use jito_bytemuck::AccountDeserialize;
use jito_jsm_core::loader::load_signer;
use jito_restaking_core::ncn::Ncn;
use ncn_program_core::{
    config::{Config as NcnConfig, ConfigAdminRole},
    error::NCNProgramError,
};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, msg, program_error::ProgramError,
    pubkey::Pubkey,
};

pub fn process_admin_set_new_admin(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    role: ConfigAdminRole,
) -> ProgramResult {
    msg!("Starting admin_set_new_admin instruction");
    let [config, ncn_account, ncn_admin, new_admin] = accounts else {
        msg!("Error: Not enough account keys provided");
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    msg!("Verifying admin signer");
    load_signer(ncn_admin, true)?;

    msg!("Loading NCN config and NCN account");
    NcnConfig::load(program_id, config, ncn_account.key, true)?;
    Ncn::load(&jito_restaking_program::id(), ncn_account, false)?;

    let mut config_data = config.try_borrow_mut_data()?;
    let config = NcnConfig::try_from_slice_unchecked_mut(&mut config_data)?;

    // Verify NCN and Admin
    msg!("Verifying NCN account");
    if config.ncn != *ncn_account.key {
        msg!("Error: Incorrect NCN account");
        return Err(NCNProgramError::IncorrectNcn.into());
    }

    let ncn_data = ncn_account.data.borrow();
    let ncn = Ncn::try_from_slice_unchecked(&ncn_data)?;

    msg!("Verifying NCN admin");
    if ncn.admin != *ncn_admin.key {
        msg!("Error: Incorrect NCN admin");
        return Err(NCNProgramError::IncorrectNcnAdmin.into());
    }

    match role {
        ConfigAdminRole::TieBreakerAdmin => {
            msg!(
                "Setting new tie breaker admin from {:?} to {:?}",
                config.tie_breaker_admin,
                new_admin.key
            );
            config.tie_breaker_admin = *new_admin.key;
            msg!("Successfully set tie breaker admin to {:?}", new_admin.key);
        }
    }

    msg!("Successfully completed admin_set_new_admin instruction");
    Ok(())
}
