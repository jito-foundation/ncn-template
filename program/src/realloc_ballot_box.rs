use jito_bytemuck::{AccountDeserialize, Discriminator};
use jito_jsm_core::loader::load_system_program;
use jito_restaking_core::ncn::Ncn;
use ncn_program_core::{
    account_payer::AccountPayer, ballot_box::BallotBox, config::Config as NcnConfig,
    epoch_state::EpochState, utils::get_new_size,
};
use solana_program::{
    account_info::AccountInfo, clock::Clock, entrypoint::ProgramResult, msg,
    program_error::ProgramError, pubkey::Pubkey, sysvar::Sysvar,
};

/// Reallocates the ballot box account to its full size.
///
/// ### Parameters:
/// - `epoch`: The target epoch
///
/// ### Accounts:
/// 1. `[writable]` epoch_state: The epoch state account for the target epoch
/// 2. `[writable]` ballot_box: The ballot box to resize
/// 3. `[]` ncn: The NCN account
/// 4. `[writable, signer]` account_payer: Account paying for reallocation
/// 5. `[]` system_program: Solana System Program
pub fn process_realloc_ballot_box(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    epoch: u64,
) -> ProgramResult {
    let [epoch_state, ncn_config, ballot_box, ncn, account_payer, system_program] = accounts else {
        msg!("Error: Not enough account keys provided");
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    load_system_program(system_program)?;
    Ncn::load(&jito_restaking_program::id(), ncn, false)?;
    EpochState::load(program_id, epoch_state, ncn.key, epoch, false)?;
    NcnConfig::load(program_id, ncn_config, ncn.key, false)?;
    AccountPayer::load(program_id, account_payer, ncn.key, true)?;

    let (ballot_box_pda, ballot_box_bump, _) =
        BallotBox::find_program_address(program_id, ncn.key, epoch);

    if ballot_box_pda != *ballot_box.key {
        msg!("Error: Ballot box account is not at the correct PDA");
        return Err(ProgramError::InvalidAccountData);
    }

    let ballot_box_size = ballot_box.data_len();
    if ballot_box_size < BallotBox::SIZE {
        let new_size = get_new_size(ballot_box_size, BallotBox::SIZE)?;
        AccountPayer::pay_and_realloc(program_id, ncn.key, account_payer, ballot_box, new_size)?;
    } else {
        msg!("Ballot box size is sufficient, no reallocation needed");
    }

    let should_initialize = ballot_box.data_len() >= BallotBox::SIZE
        && ballot_box.try_borrow_data()?[0] != BallotBox::DISCRIMINATOR;

    if should_initialize {
        let mut ballot_box_data = ballot_box.try_borrow_mut_data()?;
        ballot_box_data[0] = BallotBox::DISCRIMINATOR;
        let ballot_box_account = BallotBox::try_from_slice_unchecked_mut(&mut ballot_box_data)?;
        ballot_box_account.initialize(ncn.key, epoch, ballot_box_bump, Clock::get()?.slot);

        {
            let mut epoch_state_data = epoch_state.try_borrow_mut_data()?;
            let epoch_state_account =
                EpochState::try_from_slice_unchecked_mut(&mut epoch_state_data)?;
            epoch_state_account.update_realloc_ballot_box();
        }
    } else {
        msg!("Ballot box already initialized, skipping initialization");
    }

    Ok(())
}
