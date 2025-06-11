use crate::{
    getters::{get_account, get_is_epoch_completed, get_ncn_program_config},
    handler::CliHandler,
};
use anyhow::{anyhow, Ok, Result};
use jito_bytemuck::AccountDeserialize;

use ncn_program_core::epoch_state::{EpochState, State};
use solana_sdk::pubkey::Pubkey;

/// Represents the current state of an operator/keeper in the NCN system
/// Tracks the operator's current epoch state and completion status
#[derive(Default)]
pub struct KeeperState {
    /// Current epoch number being processed
    pub epoch: u64,
    /// On-chain address of the epoch state account
    pub epoch_state_address: Pubkey,
    /// Deserialized epoch state data, if available
    pub epoch_state: Option<Box<EpochState>>,
    /// Current state of the epoch (Vote, PostVoteCooldown, etc.)
    pub current_state: Option<State>,
    /// Flag indicating whether the current epoch has been completed by this operator
    pub is_epoch_completed: bool,
}

impl KeeperState {
    /// Initializes or updates the keeper state for a specific epoch
    ///
    /// Fetches the epoch state address and updates the internal state
    ///
    /// # Arguments
    /// * `handler` - CLI handler for RPC communication
    /// * `epoch` - Target epoch to fetch state for
    pub async fn fetch(&mut self, handler: &CliHandler, epoch: u64) -> Result<()> {
        let ncn = *handler.ncn()?;

        // Derive the on-chain address for the epoch state account
        let (epoch_state_address, _, _) =
            EpochState::find_program_address(&handler.ncn_program_id, &ncn, epoch);
        self.epoch_state_address = epoch_state_address;

        // Update the epoch number to ensure consistency
        self.epoch = epoch;

        // Update the epoch state data from the blockchain
        self.update_epoch_state(handler).await?;

        Ok(())
    }

    /// Updates the epoch state data from the blockchain
    ///
    /// Fetches the latest epoch state data and updates completion status
    ///
    /// # Arguments
    /// * `handler` - CLI handler for RPC communication
    pub async fn update_epoch_state(&mut self, handler: &CliHandler) -> Result<()> {
        // Check if this epoch is already completed for this operator
        let is_epoch_completed = get_is_epoch_completed(handler, self.epoch).await?;
        if is_epoch_completed {
            self.is_epoch_completed = true;
        } else {
            self.is_epoch_completed = false;
        }

        // Fetch the raw account data for the epoch state
        let raw_account = get_account(handler, &self.epoch_state_address).await?;

        // If the account doesn't exist, clear the epoch state
        if raw_account.is_none() {
            self.epoch_state = None;
            return Ok(());
        }

        let raw_account = raw_account.unwrap();

        // Validate the account data size
        if raw_account.data.len() < EpochState::SIZE {
            self.epoch_state = None;
            return Ok(());
        }

        // Deserialize the account data into an EpochState
        let account = Box::new(*EpochState::try_from_slice_unchecked(
            raw_account.data.as_slice(),
        )?);
        self.epoch_state = Some(account);

        // Update the current state based on the new epoch state
        self.update_current_state(handler).await?;

        Ok(())
    }

    /// Returns a reference to the current epoch state
    ///
    /// # Returns
    /// * Reference to the EpochState or an error if not available
    pub fn epoch_state(&self) -> Result<&EpochState> {
        self.epoch_state
            .as_ref()
            .map(|boxed| boxed.as_ref())
            .ok_or_else(|| anyhow!("Epoch state does not exist"))
    }

    /// Updates the current state based on the latest blockchain data
    ///
    /// Determines the current state (Vote, PostVoteCooldown, etc.) based on
    /// the current slot, epoch schedule, and program configuration
    ///
    /// # Arguments
    /// * `handler` - CLI handler for RPC communication
    pub async fn update_current_state(&mut self, handler: &CliHandler) -> Result<()> {
        let rpc_client = handler.rpc_client();
        let current_slot = rpc_client.get_epoch_info().await?.absolute_slot;
        let epoch_schedule = rpc_client.get_epoch_schedule().await?;

        // Get program configuration parameters needed for state determination
        let (valid_slots_after_consensus, epochs_after_consensus_before_close) = {
            let config = get_ncn_program_config(handler).await?;
            (
                config.valid_slots_after_consensus(),
                config.epochs_after_consensus_before_close(),
            )
        };

        let epoch_state = self.epoch_state()?;

        // Calculate the current state based on program parameters and current slot
        let state = epoch_state.current_state(
            &epoch_schedule,
            valid_slots_after_consensus,
            epochs_after_consensus_before_close,
            current_slot,
        );

        self.current_state = Some(state?);

        Ok(())
    }

    /// Returns the current state of the epoch
    ///
    /// # Returns
    /// * The current State enum value or an error if not available
    pub fn current_state(&self) -> Result<State> {
        let state = self
            .current_state
            .as_ref()
            .ok_or_else(|| anyhow!("Current state does not exist"))?;

        Ok(*state)
    }
}
