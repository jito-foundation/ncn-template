use crate::{
    getters::{get_account, get_is_epoch_completed, get_ncn_program_config},
    handler::CliHandler,
};
use anyhow::{anyhow, Ok, Result};
use jito_bytemuck::AccountDeserialize;

use ncn_program_core::epoch_state::{EpochState, State};
use solana_sdk::pubkey::Pubkey;

#[derive(Default)]
pub struct KeeperState {
    pub epoch: u64,
    pub epoch_state_address: Pubkey,
    pub epoch_state: Option<Box<EpochState>>,
    pub current_state: Option<State>,
    pub is_epoch_completed: bool,
}

impl KeeperState {
    pub async fn fetch(&mut self, handler: &CliHandler, epoch: u64) -> Result<()> {
        let ncn = *handler.ncn()?;

        let (epoch_state_address, _, _) =
            EpochState::find_program_address(&handler.ncn_program_id, &ncn, epoch);
        self.epoch_state_address = epoch_state_address;

        self.update_epoch_state(handler).await?;

        // To ensure that the state is fetched for the correct epoch
        self.epoch = epoch;

        Ok(())
    }

    pub async fn update_epoch_state(&mut self, handler: &CliHandler) -> Result<()> {
        let is_epoch_completed = get_is_epoch_completed(handler, self.epoch).await?;
        if is_epoch_completed {
            self.is_epoch_completed = true;
            return Ok(());
        } else {
            self.is_epoch_completed = false;
        }

        let raw_account = get_account(handler, &self.epoch_state_address).await?;

        if raw_account.is_none() {
            self.epoch_state = None;
            return Ok(());
        }

        let raw_account = raw_account.unwrap();

        if raw_account.data.len() < EpochState::SIZE {
            self.epoch_state = None;
            return Ok(());
        }

        let account = Box::new(*EpochState::try_from_slice_unchecked(
            raw_account.data.as_slice(),
        )?);
        self.epoch_state = Some(account);

        self.update_current_state(handler).await?;

        Ok(())
    }

    pub fn epoch_state(&self) -> Result<&EpochState> {
        self.epoch_state
            .as_ref()
            .map(|boxed| boxed.as_ref())
            .ok_or_else(|| anyhow!("Epoch state does not exist"))
    }

    pub async fn update_current_state(&mut self, handler: &CliHandler) -> Result<()> {
        let rpc_client = handler.rpc_client();
        let current_slot = rpc_client.get_epoch_info().await?.absolute_slot;
        let epoch_schedule = rpc_client.get_epoch_schedule().await?;

        let (valid_slots_after_consensus, epochs_after_consensus_before_close) = {
            let config = get_ncn_program_config(handler).await?;
            (
                config.valid_slots_after_consensus(),
                config.epochs_after_consensus_before_close(),
            )
        };

        let epoch_state = self.epoch_state()?;

        let state = epoch_state.current_state(
            &epoch_schedule,
            valid_slots_after_consensus,
            epochs_after_consensus_before_close,
            current_slot,
        );

        self.current_state = Some(state?);

        Ok(())
    }

    pub fn current_state(&self) -> Result<State> {
        let state = self
            .current_state
            .as_ref()
            .ok_or_else(|| anyhow!("Current state does not exist"))?;

        Ok(*state)
    }

    pub async fn detect_stall(&mut self) -> Result<bool> {
        if self.is_epoch_completed {
            return Ok(true);
        }

        let current_state = self.current_state()?;

        if current_state == State::Vote || current_state == State::PostVoteCooldown {
            return Ok(true);
        }

        Ok(false)
    }
}
