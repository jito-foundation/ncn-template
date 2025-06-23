use jito_bytemuck::AccountDeserialize;
use jito_jsm_core::loader::load_signer;
use jito_restaking_core::ncn::Ncn;
use ncn_program_core::{
    ballot_box::BallotBox, config::Config as NcnConfig, epoch_state::EpochState,
    error::NCNProgramError,
};
use solana_program::{
    account_info::AccountInfo, clock::Clock, entrypoint::ProgramResult, msg,
    program_error::ProgramError, pubkey::Pubkey, sysvar::Sysvar,
};

/// Allows the tie-breaker admin to resolve stalled votes by selecting a winning ballot.
///
/// ### Parameters:
/// - `weather_status`: Status code for the tie-breaking vote (0=Sunny, 1=Cloudy, 2=Rainy)
/// - `epoch`: The target epoch
///
/// ### Accounts:
/// 1. `[writable]` epoch_state: The epoch state account for the target epoch
/// 2. `[]` config: NCN configuration account (named `ncn_config` in code)
/// 3. `[writable]` ballot_box: The ballot box containing votes
/// 4. `[]` ncn: The NCN account
/// 5. `[signer]` tie_breaker_admin: Admin account authorized to break ties
pub fn process_admin_set_tie_breaker(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    weather_status: u8,
    epoch: u64,
) -> ProgramResult {
    msg!("Starting admin_set_tie_breaker instruction");
    let [epoch_state, ncn_config, ballot_box, ncn, tie_breaker_admin] = accounts else {
        msg!("Error: Not enough account keys provided");
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    msg!("Loading and verifying accounts");
    EpochState::load(program_id, epoch_state, ncn.key, epoch, true)?;
    NcnConfig::load(program_id, ncn_config, ncn.key, false)?;
    BallotBox::load(program_id, ballot_box, ncn.key, epoch, true)?;
    Ncn::load(&jito_restaking_program::id(), ncn, false)?;

    msg!("Verifying tie breaker admin signer");
    load_signer(tie_breaker_admin, false)?;

    msg!("Verifying tie breaker admin authority");
    let ncn_config_data = ncn_config.data.borrow();
    let ncn_config = NcnConfig::try_from_slice_unchecked(&ncn_config_data)?;

    if ncn_config.tie_breaker_admin.ne(tie_breaker_admin.key) {
        msg!("Error: Invalid tie breaker admin");
        return Err(NCNProgramError::TieBreakerAdminInvalid.into());
    }

    msg!("Updating ballot box with tie breaker vote");
    let mut ballot_box_data = ballot_box.data.borrow_mut();
    let ballot_box_account = BallotBox::try_from_slice_unchecked_mut(&mut ballot_box_data)?;

    let clock = Clock::get()?;
    let current_epoch = clock.epoch;
    msg!("Current epoch: {}", current_epoch);

    msg!(
        "Setting tie breaker ballot with weather status: {}",
        weather_status
    );
    ballot_box_account.set_tie_breaker_ballot(
        weather_status,
        current_epoch,
        ncn_config.epochs_before_stall(),
    )?;

    msg!("Updating epoch state");
    {
        let slot = clock.slot;
        let mut epoch_state_data = epoch_state.try_borrow_mut_data()?;
        let epoch_state_account = EpochState::try_from_slice_unchecked_mut(&mut epoch_state_data)?;
        let consensus_reached = ballot_box_account.is_consensus_reached();
        msg!("Consensus reached: {}", consensus_reached);
        epoch_state_account.update_set_tie_breaker(consensus_reached, slot)?;
    }

    Ok(())
}
