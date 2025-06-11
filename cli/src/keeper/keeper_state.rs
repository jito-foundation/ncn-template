use crate::{
    getters::{get_account, get_is_epoch_completed, get_ncn_program_config},
    handler::CliHandler,
};
use anyhow::{anyhow, Ok, Result};
use jito_bytemuck::AccountDeserialize;

use ncn_program_core::epoch_state::{EpochState, State};
use solana_sdk::pubkey::Pubkey;

/// Manages the state of the keeper for a specific epoch
///
/// The KeeperState tracks the current epoch being processed, the on-chain epoch state,
/// and provides methods to update and query this state. It serves as the central
/// state management for the keeper's operations.
#[derive(Default)]
pub struct KeeperState {
    /// The epoch number this keeper state is tracking
    pub epoch: u64,
    /// The on-chain address of the EpochState account for this epoch
    pub epoch_state_address: Pubkey,
    /// The deserialized EpochState account data, if it exists
    pub epoch_state: Option<Box<EpochState>>,
    /// The current state of the epoch (SetWeight, Snapshot, Vote, etc.)
    pub current_state: Option<State>,
    /// Whether this epoch has been completed (closed)
    pub is_epoch_completed: bool,
}

impl KeeperState {
    /// Initializes the keeper state for a specific epoch
    ///
    /// This method:
    /// 1. Calculates the epoch state address for the given epoch
    /// 2. Updates the epoch state from on-chain data
    /// 3. Sets the epoch number
    ///
    /// # Arguments
    /// * `handler` - The CLI handler containing RPC client and configuration
    /// * `epoch` - The epoch number to track
    pub async fn fetch(&mut self, handler: &CliHandler, epoch: u64) -> Result<()> {
        let ncn = *handler.ncn()?;

        // Calculate the program-derived address for the epoch state account
        let (epoch_state_address, _, _) =
            EpochState::find_program_address(&handler.ncn_program_id, &ncn, epoch);
        self.epoch_state_address = epoch_state_address;

        // Fetch the current state from on-chain
        self.update_epoch_state(handler).await?;

        // Store the epoch number to ensure state consistency
        self.epoch = epoch;

        Ok(())
    }

    /// Updates the epoch state by fetching the latest data from the blockchain
    ///
    /// This method:
    /// 1. Checks if the epoch is completed
    /// 2. Fetches the epoch state account data
    /// 3. Deserializes the account data if valid
    /// 4. Updates the current state based on blockchain conditions
    ///
    /// # Arguments
    /// * `handler` - The CLI handler for blockchain interactions
    pub async fn update_epoch_state(&mut self, handler: &CliHandler) -> Result<()> {
        // Check if this epoch has been marked as completed
        let is_epoch_completed = get_is_epoch_completed(handler, self.epoch).await?;
        if is_epoch_completed {
            self.is_epoch_completed = true;
            return Ok(());
        } else {
            self.is_epoch_completed = false;
        }

        // Fetch the raw account data for the epoch state
        let raw_account = get_account(handler, &self.epoch_state_address).await?;

        // If no account exists, the epoch state hasn't been created yet
        if raw_account.is_none() {
            self.epoch_state = None;
            return Ok(());
        }

        let raw_account = raw_account.unwrap();

        // Validate that the account has sufficient data for an EpochState
        if raw_account.data.len() < EpochState::SIZE {
            self.epoch_state = None;
            return Ok(());
        }

        // Deserialize the account data into an EpochState struct
        let account = Box::new(*EpochState::try_from_slice_unchecked(
            raw_account.data.as_slice(),
        )?);
        self.epoch_state = Some(account);

        // Update the current state based on the epoch state and blockchain conditions
        self.update_current_state(handler).await?;

        Ok(())
    }

    /// Returns a reference to the epoch state, or an error if it doesn't exist
    ///
    /// # Returns
    /// A reference to the EpochState if it exists, otherwise an error
    pub fn epoch_state(&self) -> Result<&EpochState> {
        self.epoch_state
            .as_ref()
            .map(|boxed| boxed.as_ref())
            .ok_or_else(|| anyhow!("Epoch state does not exist"))
    }

    /// Updates the current state based on blockchain conditions and epoch state
    ///
    /// This method determines what phase the epoch is currently in by considering:
    /// - Current slot and epoch schedule
    /// - Configuration parameters (consensus timing, etc.)
    /// - The epoch state's progress tracking
    ///
    /// # Arguments
    /// * `handler` - The CLI handler for blockchain queries
    pub async fn update_current_state(&mut self, handler: &CliHandler) -> Result<()> {
        let rpc_client = handler.rpc_client();
        let current_slot = rpc_client.get_epoch_info().await?.absolute_slot;
        let epoch_schedule = rpc_client.get_epoch_schedule().await?;

        // Get configuration parameters that affect state transitions
        let (valid_slots_after_consensus, epochs_after_consensus_before_close) = {
            let config = get_ncn_program_config(handler).await?;
            (
                config.valid_slots_after_consensus(),
                config.epochs_after_consensus_before_close(),
            )
        };

        let epoch_state = self.epoch_state()?;

        // Calculate the current state based on timing and progress
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
    /// The current State enum value (SetWeight, Snapshot, Vote, PostVoteCooldown, or Close)
    pub fn current_state(&self) -> Result<State> {
        let state = self
            .current_state
            .as_ref()
            .ok_or_else(|| anyhow!("Current state does not exist"))?;

        Ok(*state)
    }

    /// Detects if the epoch has stalled and should be progressed
    ///
    /// An epoch is considered stalled if:
    /// 1. It has been completed
    /// 2. It's in the Vote or PostVoteCooldown state (waiting for external actions)
    ///
    /// # Returns
    /// `true` if the epoch should be progressed to the next one, `false` otherwise
    pub async fn detect_stall(&mut self) -> Result<bool> {
        // If epoch is completed, it's considered stalled (should move to next epoch)
        if self.is_epoch_completed {
            return Ok(true);
        }

        let current_state = self.current_state()?;

        // Vote and PostVoteCooldown states can stall waiting for operator actions
        if current_state == State::Vote || current_state == State::PostVoteCooldown {
            return Ok(true);
        }

        Ok(false)
    }
}
