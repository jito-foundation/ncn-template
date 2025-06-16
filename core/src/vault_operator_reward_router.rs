use core::{fmt, mem::size_of};

use bytemuck::{Pod, Zeroable};
use jito_bytemuck::{
    types::{PodU16, PodU64},
    AccountDeserialize, Discriminator,
};
use jito_vault_core::MAX_BPS;
use shank::{ShankAccount, ShankType};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, msg, program::invoke_signed,
    program_error::ProgramError, pubkey::Pubkey, rent::Rent, system_instruction, system_program,
    sysvar::Sysvar,
};
use spl_math::precise_number::PreciseNumber;

use crate::{
    constants::MAX_VAULTS, discriminators::Discriminators, epoch_snapshot::OperatorSnapshot,
    error::NCNProgramError, loaders::check_load,
};

/// Operator Vault Reward Router - Routes rewards from operators to their associated vaults
///
/// This router handles the final stage of reward distribution where operator rewards
/// are further distributed to the vaults they operate. The distribution is based on:
/// 1. Operator takes their fee percentage first
/// 2. Remaining rewards are distributed to vaults proportionally by stake weight
///
/// The router supports partial routing through iterations to handle large numbers of vaults
/// without hitting transaction limits.
///
/// PDA: ["operator_vault_reward_route", OPERATOR, NCN, EPOCH]
#[derive(Debug, Clone, Copy, Zeroable, Pod, AccountDeserialize, ShankAccount)]
#[repr(C)]
pub struct OperatorVaultRewardRouter {
    /// The operator the router is associated with
    operator: Pubkey,
    /// The NCN the router is associated with
    ncn: Pubkey,
    /// The epoch the router is associated with
    epoch: PodU64,
    /// The bump seed for the PDA
    bump: u8,
    /// The slot the router was created
    slot_created: PodU64,
    /// The operator's index within the NCN
    ncn_operator_index: PodU64,
    /// The total rewards that have been routed (in lamports) - cumulative amount ever processed
    total_rewards: PodU64,
    /// The rewards in the reward pool (in lamports) - awaiting distribution
    reward_pool: PodU64,
    /// The rewards that have been processed (in lamports) - moved out of reward pool
    rewards_processed: PodU64,
    /// Rewards allocated to the operator (in lamports) - operator's fee portion
    operator_rewards: PodU64,

    // Routing state - enables recovery from partial routing operations
    /// The last rewards amount being processed during routing (for resuming partial operations)
    last_rewards_to_process: PodU64,
    /// The last vault operator delegation index processed during routing
    last_vault_operator_delegation_index: PodU16,

    /// Individual vault reward routes - tracks rewards per vault (limited to 64 vaults)
    vault_reward_routes: [VaultRewardRoute; 64],
}

impl Discriminator for OperatorVaultRewardRouter {
    const DISCRIMINATOR: u8 = Discriminators::OperatorVaultRewardRouter as u8;
}

impl OperatorVaultRewardRouter {
    pub const SIZE: usize = 8 + size_of::<Self>();

    /// Sentinel values indicating no partial routing is in progress
    pub const NO_LAST_REWARDS_TO_PROCESS: u64 = u64::MAX;
    pub const NO_LAST_VAULT_OPERATION_DELEGATION_INDEX: u16 = u16::MAX;

    /// Maximum iterations per routing call to prevent transaction timeout
    pub const MAX_ROUTE_NCN_ITERATIONS: u16 = 30;

    pub const OPERATOR_VAULT_REWARD_ROUTE_SEED: &'static [u8] = b"operator_vault_reward_route";

    /// Creates a new operator vault reward router
    pub fn new(
        operator: &Pubkey,
        operator_ncn_index: u64,
        ncn: &Pubkey,
        epoch: u64,
        bump: u8,
        slot_created: u64,
    ) -> Self {
        Self {
            operator: *operator,
            ncn: *ncn,
            epoch: PodU64::from(epoch),
            bump,
            slot_created: PodU64::from(slot_created),
            ncn_operator_index: PodU64::from(operator_ncn_index),
            total_rewards: PodU64::from(0),
            reward_pool: PodU64::from(0),
            rewards_processed: PodU64::from(0),
            operator_rewards: PodU64::from(0),
            last_rewards_to_process: PodU64::from(Self::NO_LAST_REWARDS_TO_PROCESS),
            last_vault_operator_delegation_index: PodU16::from(
                Self::NO_LAST_VAULT_OPERATION_DELEGATION_INDEX,
            ),
            vault_reward_routes: [VaultRewardRoute::default(); MAX_VAULTS],
        }
    }

    /// Generates PDA seeds for the operator vault reward router
    pub fn seeds(operator: &Pubkey, ncn: &Pubkey, epoch: u64) -> Vec<Vec<u8>> {
        Vec::from_iter(
            [
                Self::OPERATOR_VAULT_REWARD_ROUTE_SEED.to_vec(),
                operator.to_bytes().to_vec(),
                ncn.to_bytes().to_vec(),
                epoch.to_le_bytes().to_vec(),
            ]
            .iter()
            .cloned(),
        )
    }

    /// Finds the program address for the operator vault reward router PDA
    pub fn find_program_address(
        program_id: &Pubkey,
        operator: &Pubkey,
        ncn: &Pubkey,
        epoch: u64,
    ) -> (Pubkey, u8, Vec<Vec<u8>>) {
        let seeds = Self::seeds(operator, ncn, epoch);
        let seeds_iter: Vec<_> = seeds.iter().map(|s| s.as_slice()).collect();
        let (pda, bump) = Pubkey::find_program_address(&seeds_iter, program_id);
        (pda, bump, seeds)
    }

    /// Validates that the account matches expected PDA and discriminator
    pub fn load(
        program_id: &Pubkey,
        account: &AccountInfo,
        operator: &Pubkey,
        ncn: &Pubkey,
        epoch: u64,
        expect_writable: bool,
    ) -> Result<(), ProgramError> {
        let expected_pda = Self::find_program_address(program_id, operator, ncn, epoch).0;
        check_load(
            program_id,
            account,
            &expected_pda,
            Some(Self::DISCRIMINATOR),
            expect_writable,
        )
    }

    /// Loads the account for closing by extracting operator from account data
    pub fn load_to_close(
        program_id: &Pubkey,
        account_to_close: &AccountInfo,
        ncn: &Pubkey,
        epoch: u64,
    ) -> Result<(), ProgramError> {
        let account_data = account_to_close.try_borrow_data()?;
        let account_struct = Self::try_from_slice_unchecked(&account_data)?;
        let operator = *account_struct.operator();

        Self::load(program_id, account_to_close, &operator, ncn, epoch, true)
    }

    // ----------------- GETTERS -----------------

    pub const fn operator(&self) -> &Pubkey {
        &self.operator
    }

    pub const fn ncn(&self) -> &Pubkey {
        &self.ncn
    }

    pub fn epoch(&self) -> u64 {
        self.epoch.into()
    }

    pub fn ncn_operator_index(&self) -> u64 {
        self.ncn_operator_index.into()
    }

    pub fn slot_created(&self) -> u64 {
        self.slot_created.into()
    }

    pub const fn vault_reward_routes(&self) -> &[VaultRewardRoute] {
        &self.vault_reward_routes
    }

    // ----------------- ROUTE STATE TRACKING --------------

    /// Gets the last rewards amount being processed during partial routing
    pub fn last_rewards_to_process(&self) -> u64 {
        self.last_rewards_to_process.into()
    }

    /// Gets the last vault operator delegation index processed during partial routing
    pub fn last_vault_operator_delegation_index(&self) -> u16 {
        self.last_vault_operator_delegation_index.into()
    }

    /// Resumes routing from the last saved state if routing was interrupted
    /// Returns (rewards_to_process, starting_vault_index)
    pub fn resume_routing_state(&mut self, rewards_to_process: u64) -> (u64, usize) {
        if !self.still_routing() {
            return (rewards_to_process, 0);
        }

        (
            self.last_rewards_to_process(),
            self.last_vault_operator_delegation_index() as usize,
        )
    }

    /// Saves the current routing state for resumption if interrupted
    pub fn save_routing_state(
        &mut self,
        rewards_to_process: u64,
        vault_operator_delegation_index: usize,
    ) {
        self.last_rewards_to_process = PodU64::from(rewards_to_process);
        self.last_vault_operator_delegation_index =
            PodU16::from(vault_operator_delegation_index as u16);
    }

    /// Resets routing state to indicate no partial routing is in progress
    pub fn reset_routing_state(&mut self) {
        self.last_rewards_to_process = PodU64::from(Self::NO_LAST_REWARDS_TO_PROCESS);
        self.last_vault_operator_delegation_index =
            PodU16::from(Self::NO_LAST_VAULT_OPERATION_DELEGATION_INDEX);
    }

    /// Checks if routing is still in progress (was interrupted)
    pub fn still_routing(&self) -> bool {
        self.last_rewards_to_process() != Self::NO_LAST_REWARDS_TO_PROCESS
            || self.last_vault_operator_delegation_index()
                != Self::NO_LAST_VAULT_OPERATION_DELEGATION_INDEX
    }

    // ------------------------ ROUTING ------------------------

    /// Routes incoming MEV rewards from account balance to the reward pool
    /// This is the entry point for new rewards coming into the operator router
    pub fn route_incoming_rewards(
        &mut self,
        rent_cost: u64,
        account_balance: u64,
    ) -> Result<(), NCNProgramError> {
        let total_rewards = self.total_rewards_in_transit()?;

        // Calculate new rewards as difference between current balance and known rewards
        let incoming_rewards = account_balance
            .checked_sub(total_rewards)
            .ok_or(NCNProgramError::ArithmeticUnderflowError)?;

        // Subtract rent cost to get distributable rewards
        let rewards_to_route = incoming_rewards
            .checked_sub(rent_cost)
            .ok_or(NCNProgramError::ArithmeticUnderflowError)?;

        self.route_to_reward_pool(rewards_to_route)?;

        Ok(())
    }

    /// Routes operator fee rewards based on the operator's fee percentage
    /// This is the first phase of reward distribution - operator takes their fee
    pub fn route_operator_rewards(
        &mut self,
        operator_snapshot: &OperatorSnapshot,
    ) -> Result<(), NCNProgramError> {
        let rewards_to_process: u64 = self.reward_pool();

        // Calculate and route operator fee rewards
        {
            let operator_fee_bps = operator_snapshot.operator_fee_bps();
            let operator_rewards =
                Self::calculate_operator_reward(operator_fee_bps as u64, rewards_to_process)?;

            self.route_from_reward_pool(operator_rewards)?;
            self.route_to_operator_rewards(operator_rewards)?;
        }

        Ok(())
    }

    /// Routes remaining rewards to vaults based on their stake weights
    /// This is the second phase of reward distribution that can be done iteratively
    pub fn route_reward_pool(
        &mut self,
        operator_snapshot: &OperatorSnapshot,
        max_iterations: u16,
    ) -> Result<(), NCNProgramError> {
        {
            let operator_stake_weight = operator_snapshot.stake_weights();
            let rewards_to_process: u64 = self.reward_pool();

            let (rewards_to_process, starting_vault_operator_delegation_index) =
                self.resume_routing_state(rewards_to_process);

            if rewards_to_process == 0 {
                return Ok(());
            }

            let mut iterations: u16 = 0;
            // Always have at least 1 iteration to make progress
            let max_iterations = max_iterations.max(1);

            // Iterate through vault operator delegations and distribute rewards proportionally
            for vault_operator_delegation_index in starting_vault_operator_delegation_index
                ..operator_snapshot.vault_operator_stake_weight().len()
            {
                let vault_operator_delegation = operator_snapshot.vault_operator_stake_weight()
                    [vault_operator_delegation_index];

                // Track iterations to prevent transaction timeout
                {
                    iterations = iterations
                        .checked_add(1)
                        .ok_or(NCNProgramError::ArithmeticOverflow)?;

                    if iterations > max_iterations {
                        msg!(
                            "Reached max iterations, saving state and exiting {}/{}",
                            rewards_to_process,
                            vault_operator_delegation_index
                        );
                        self.save_routing_state(
                            rewards_to_process,
                            vault_operator_delegation_index,
                        );
                        return Ok(());
                    }
                }

                let vault = vault_operator_delegation.vault();

                let vault_reward_stake_weight =
                    vault_operator_delegation.stake_weights().stake_weight();

                let operator_reward_stake_weight = operator_stake_weight.stake_weight();

                // Calculate proportional reward based on vault's stake weight
                let vault_reward = Self::calculate_vault_reward(
                    vault_reward_stake_weight,
                    operator_reward_stake_weight,
                    rewards_to_process,
                )?;

                self.route_from_reward_pool(vault_reward)?;
                self.route_to_vault_reward_route(vault, vault_reward)?;
            }

            self.reset_routing_state();
        }

        // Operator gets any remainder due to rounding
        {
            let leftover_rewards = self.reward_pool();

            self.route_from_reward_pool(leftover_rewards)?;
            self.route_to_operator_rewards(leftover_rewards)?;
        }

        Ok(())
    }

    // ------------------------ CALCULATIONS ------------------------

    /// Calculates operator reward based on their fee percentage
    /// Formula: (operator_fee_bps / MAX_BPS) * total_rewards
    fn calculate_operator_reward(
        fee_bps: u64,
        rewards_to_process: u64,
    ) -> Result<u64, NCNProgramError> {
        if fee_bps == 0 || rewards_to_process == 0 {
            return Ok(0);
        }

        let precise_operator_fee_bps =
            PreciseNumber::new(fee_bps as u128).ok_or(NCNProgramError::NewPreciseNumberError)?;

        let precise_max_bps =
            PreciseNumber::new(MAX_BPS as u128).ok_or(NCNProgramError::NewPreciseNumberError)?;

        let precise_rewards_to_process = PreciseNumber::new(rewards_to_process as u128)
            .ok_or(NCNProgramError::NewPreciseNumberError)?;

        let precise_operator_rewards = precise_rewards_to_process
            .checked_mul(&precise_operator_fee_bps)
            .and_then(|x| x.checked_div(&precise_max_bps))
            .ok_or(NCNProgramError::ArithmeticOverflow)?;

        let floored_precise_operator_rewards = precise_operator_rewards
            .floor()
            .ok_or(NCNProgramError::ArithmeticFloorError)?;

        let operator_rewards_u128: u128 = floored_precise_operator_rewards
            .to_imprecise()
            .ok_or(NCNProgramError::CastToImpreciseNumberError)?;
        let operator_rewards: u64 = operator_rewards_u128
            .try_into()
            .map_err(|_| NCNProgramError::CastToU64Error)?;

        Ok(operator_rewards)
    }

    /// Calculates proportional vault reward based on its stake weight
    /// Formula: (vault_stake_weight / total_operator_stake_weight) * total_rewards
    fn calculate_vault_reward(
        vault_reward_stake_weight: u128,
        operator_reward_stake_weight: u128,
        rewards_to_process: u64,
    ) -> Result<u64, NCNProgramError> {
        if operator_reward_stake_weight == 0 || rewards_to_process == 0 {
            return Ok(0);
        }

        let precise_rewards_to_process = PreciseNumber::new(rewards_to_process as u128)
            .ok_or(NCNProgramError::NewPreciseNumberError)?;

        let precise_vault_reward_stake_weight = PreciseNumber::new(vault_reward_stake_weight)
            .ok_or(NCNProgramError::NewPreciseNumberError)?;

        let precise_operator_reward_stake_weight = PreciseNumber::new(operator_reward_stake_weight)
            .ok_or(NCNProgramError::NewPreciseNumberError)?;

        let precise_vault_reward = precise_rewards_to_process
            .checked_mul(&precise_vault_reward_stake_weight)
            .and_then(|x| x.checked_div(&precise_operator_reward_stake_weight))
            .ok_or(NCNProgramError::ArithmeticOverflow)?;

        let floored_precise_vault_reward = precise_vault_reward
            .floor()
            .ok_or(NCNProgramError::ArithmeticFloorError)?;

        let vault_reward_u128: u128 = floored_precise_vault_reward
            .to_imprecise()
            .ok_or(NCNProgramError::CastToImpreciseNumberError)?;

        let vault_reward: u64 = vault_reward_u128
            .try_into()
            .map_err(|_| NCNProgramError::CastToU64Error)?;

        Ok(vault_reward)
    }

    // ------------------------ REWARD POOL ------------------------

    /// Calculates total rewards currently being processed (reward pool + processed)
    pub fn total_rewards_in_transit(&self) -> Result<u64, NCNProgramError> {
        let total_rewards = self
            .reward_pool()
            .checked_add(self.rewards_processed())
            .ok_or(NCNProgramError::ArithmeticOverflow)?;

        Ok(total_rewards)
    }

    /// Calculates minimum rent cost for this account
    pub fn rent_cost(&self, rent: &Rent) -> Result<u64, NCNProgramError> {
        let size = 8_u64
            .checked_add(size_of::<Self>() as u64)
            .ok_or(NCNProgramError::ArithmeticOverflow)?;

        Ok(rent.minimum_balance(size as usize))
    }

    pub fn total_rewards(&self) -> u64 {
        self.total_rewards.into()
    }

    pub fn reward_pool(&self) -> u64 {
        self.reward_pool.into()
    }

    /// Adds rewards to the reward pool and updates total rewards counter
    pub fn route_to_reward_pool(&mut self, rewards: u64) -> Result<(), NCNProgramError> {
        if rewards == 0 {
            return Ok(());
        }

        self.total_rewards = PodU64::from(
            self.total_rewards()
                .checked_add(rewards)
                .ok_or(NCNProgramError::ArithmeticOverflow)?,
        );

        self.reward_pool = PodU64::from(
            self.reward_pool()
                .checked_add(rewards)
                .ok_or(NCNProgramError::ArithmeticOverflow)?,
        );
        Ok(())
    }

    /// Moves rewards out of the reward pool and marks them as processed
    pub fn route_from_reward_pool(&mut self, rewards: u64) -> Result<(), NCNProgramError> {
        if rewards == 0 {
            return Ok(());
        }

        self.reward_pool = PodU64::from(
            self.reward_pool()
                .checked_sub(rewards)
                .ok_or(NCNProgramError::ArithmeticUnderflowError)?,
        );

        self.increment_rewards_processed(rewards)?;

        Ok(())
    }

    // ------------------------ REWARDS PROCESSED ------------------------

    pub fn rewards_processed(&self) -> u64 {
        self.rewards_processed.into()
    }

    /// Increments the counter of rewards that have been processed (moved out of reward pool)
    pub fn increment_rewards_processed(&mut self, rewards: u64) -> Result<(), NCNProgramError> {
        if rewards == 0 {
            return Ok(());
        }

        self.rewards_processed = PodU64::from(
            self.rewards_processed()
                .checked_add(rewards)
                .ok_or(NCNProgramError::ArithmeticOverflow)?,
        );
        Ok(())
    }

    /// Decrements the counter of rewards processed (when rewards are distributed)
    pub fn decrement_rewards_processed(&mut self, rewards: u64) -> Result<(), NCNProgramError> {
        if rewards == 0 {
            return Ok(());
        }

        self.rewards_processed = PodU64::from(
            self.rewards_processed()
                .checked_sub(rewards)
                .ok_or(NCNProgramError::ArithmeticUnderflowError)?,
        );
        Ok(())
    }

    // ------------------------ OPERATOR REWARDS ------------------------

    pub fn operator_rewards(&self) -> u64 {
        self.operator_rewards.into()
    }

    /// Routes rewards to operator allocation
    pub fn route_to_operator_rewards(&mut self, rewards: u64) -> Result<(), NCNProgramError> {
        if rewards == 0 {
            return Ok(());
        }

        self.operator_rewards = PodU64::from(
            self.operator_rewards()
                .checked_add(rewards)
                .ok_or(NCNProgramError::ArithmeticOverflow)?,
        );

        Ok(())
    }

    /// Distributes operator rewards and updates counters
    /// Returns the amount of rewards distributed
    pub fn distribute_operator_rewards(&mut self) -> Result<u64, NCNProgramError> {
        let rewards = self.operator_rewards();

        self.operator_rewards = PodU64::from(
            self.operator_rewards()
                .checked_sub(rewards)
                .ok_or(NCNProgramError::ArithmeticUnderflowError)?,
        );

        self.decrement_rewards_processed(rewards)?;
        Ok(rewards)
    }

    // ------------------------ VAULT REWARD ROUTES ------------------------

    /// Gets the reward route for a specific vault
    pub fn vault_reward_route(&self, vault: &Pubkey) -> Result<&VaultRewardRoute, NCNProgramError> {
        for vault_reward in self.vault_reward_routes.iter() {
            if vault_reward.vault().eq(vault) {
                return Ok(vault_reward);
            }
        }
        Err(NCNProgramError::VaultRewardNotFound)
    }

    /// Routes rewards to a specific vault's reward route
    /// Creates a new route if one doesn't exist for the vault
    pub fn route_to_vault_reward_route(
        &mut self,
        vault: &Pubkey,
        rewards: u64,
    ) -> Result<(), NCNProgramError> {
        if rewards == 0 {
            return Ok(());
        }

        // Try to find existing route and increment rewards
        for vault_reward in self.vault_reward_routes.iter_mut() {
            if vault_reward.vault().eq(vault) {
                vault_reward.increment_rewards(rewards)?;
                return Ok(());
            }
        }

        // Find empty slot and create new route
        for vault_reward in self.vault_reward_routes.iter_mut() {
            if vault_reward.vault().eq(&Pubkey::default()) {
                *vault_reward = VaultRewardRoute::new(vault, rewards)?;
                return Ok(());
            }
        }

        Err(NCNProgramError::OperatorRewardListFull)
    }

    /// Distributes rewards for a specific vault and updates counters
    /// Returns the amount of rewards distributed
    pub fn distribute_vault_reward_route(
        &mut self,
        vault: &Pubkey,
    ) -> Result<u64, NCNProgramError> {
        for route in self.vault_reward_routes.iter_mut() {
            if route.vault().eq(vault) {
                let rewards = route.rewards();

                route.decrement_rewards(rewards)?;
                self.decrement_rewards_processed(rewards)?;
                return Ok(rewards);
            }
        }
        Err(NCNProgramError::OperatorRewardNotFound)
    }
}

/// Operator Vault Reward Receiver - Uninitialized account that receives rewards for operator-vault combinations
///
/// This is a simple PDA account with no data that acts as a destination for operator vault rewards.
/// It must remain uninitialized to be used as a payer in transfer instructions.
/// The account is closed after rewards are transferred to the Operator Vault Reward Router.
///
/// PDA: ["operator_vault_reward_receiver", OPERATOR, NCN, EPOCH]
pub struct OperatorVaultRewardReceiver {}

impl OperatorVaultRewardReceiver {
    pub const OPERATOR_VAULT_REWARD_RECEIVER_SEED: &'static [u8] =
        b"operator_vault_reward_receiver";

    /// Generates PDA seeds for the operator vault reward receiver
    pub fn seeds(operator: &Pubkey, ncn: &Pubkey, epoch: u64) -> Vec<Vec<u8>> {
        vec![
            Self::OPERATOR_VAULT_REWARD_RECEIVER_SEED.to_vec(),
            operator.to_bytes().to_vec(),
            ncn.to_bytes().to_vec(),
            epoch.to_le_bytes().to_vec(),
        ]
    }

    /// Finds the program address for the operator vault reward receiver PDA
    pub fn find_program_address(
        program_id: &Pubkey,
        operator: &Pubkey,
        ncn: &Pubkey,
        epoch: u64,
    ) -> (Pubkey, u8, Vec<Vec<u8>>) {
        let seeds = Self::seeds(operator, ncn, epoch);
        let (address, bump) = Pubkey::find_program_address(
            &seeds.iter().map(|s| s.as_slice()).collect::<Vec<_>>(),
            program_id,
        );
        (address, bump, seeds)
    }

    /// Validates that the account is owned by system program and matches expected PDA
    /// Operator vault reward receiver accounts are owned by system program, not the NCN program
    pub fn load(
        program_id: &Pubkey,
        account: &AccountInfo,
        operator: &Pubkey,
        ncn: &Pubkey,
        epoch: u64,
        expect_writable: bool,
    ) -> Result<(), ProgramError> {
        let system_program_id = system_program::id();
        let expected_pda = Self::find_program_address(program_id, operator, ncn, epoch).0;
        check_load(
            &system_program_id,
            account,
            &expected_pda,
            None,
            expect_writable,
        )
    }

    /// Closes the operator vault reward receiver account and transfers remaining lamports
    ///
    /// This function transfers all lamports above minimum rent to the DAO wallet,
    /// then transfers the minimum rent to the account payer (for rent recovery).
    #[inline(always)]
    #[allow(clippy::too_many_arguments)]
    pub fn close<'a, 'info>(
        program_id: &Pubkey,
        operator: &Pubkey,
        ncn: &Pubkey,
        epoch: u64,
        operator_vault_reward_receiver: &'a AccountInfo<'info>,
        dao_wallet: &'a AccountInfo<'info>,
        account_payer: &'a AccountInfo<'info>,
    ) -> ProgramResult {
        let min_rent = Rent::get()?.minimum_balance(0);

        // Transfer excess lamports to DAO wallet
        let delta_lamports = operator_vault_reward_receiver
            .lamports()
            .saturating_sub(min_rent);
        if delta_lamports > 0 {
            Self::transfer(
                program_id,
                operator,
                ncn,
                epoch,
                operator_vault_reward_receiver,
                dao_wallet,
                delta_lamports,
            )?;
        }

        // Transfer minimum rent back to payer
        Self::transfer(
            program_id,
            operator,
            ncn,
            epoch,
            operator_vault_reward_receiver,
            account_payer,
            min_rent,
        )
    }

    /// Transfers lamports from the operator vault reward receiver using PDA authority
    #[inline(always)]
    #[allow(clippy::too_many_arguments)]
    pub fn transfer<'a, 'info>(
        program_id: &Pubkey,
        operator: &Pubkey,
        ncn: &Pubkey,
        epoch: u64,
        operator_vault_reward_receiver: &'a AccountInfo<'info>,
        to: &'a AccountInfo<'info>,
        lamports: u64,
    ) -> ProgramResult {
        let (
            operator_vault_reward_receiver_address,
            operator_vault_reward_receiver_bump,
            mut operator_vault_reward_receiver_seeds,
        ) = Self::find_program_address(program_id, operator, ncn, epoch);
        operator_vault_reward_receiver_seeds.push(vec![operator_vault_reward_receiver_bump]);

        if operator_vault_reward_receiver_address.ne(operator_vault_reward_receiver.key) {
            msg!("Incorrect operator vault reward receiver PDA");
            return Err(ProgramError::InvalidAccountData);
        }

        invoke_signed(
            &system_instruction::transfer(
                &operator_vault_reward_receiver_address,
                to.key,
                lamports,
            ),
            &[operator_vault_reward_receiver.clone(), to.clone()],
            &[operator_vault_reward_receiver_seeds
                .iter()
                .map(|seed| seed.as_slice())
                .collect::<Vec<&[u8]>>()
                .as_slice()],
        )?;
        Ok(())
    }
}

/// Individual vault reward route - tracks rewards for a specific vault
/// This struct stores the allocation of rewards for a vault before distribution
#[derive(Default, Debug, Clone, PartialEq, Eq, Copy, Zeroable, ShankType, Pod)]
#[repr(C)]
pub struct VaultRewardRoute {
    /// The vault pubkey that will receive rewards
    vault: Pubkey,
    /// The amount of rewards allocated to this vault (in lamports)
    rewards: PodU64,
}

impl VaultRewardRoute {
    /// Creates a new vault reward route with initial reward amount
    pub fn new(vault: &Pubkey, rewards: u64) -> Result<Self, NCNProgramError> {
        Ok(Self {
            vault: *vault,
            rewards: PodU64::from(rewards),
        })
    }

    /// Gets the vault pubkey for this route
    pub const fn vault(&self) -> Pubkey {
        self.vault
    }

    /// Gets the reward amount for this route
    pub fn rewards(&self) -> u64 {
        self.rewards.into()
    }

    /// Checks if this route slot is empty (default vault)
    pub fn is_empty(&self) -> bool {
        self.vault.eq(&Pubkey::default())
    }

    /// Checks if this route has any rewards allocated
    pub fn has_rewards(&self) -> bool {
        self.rewards() > 0
    }

    /// Sets the reward amount for this route
    fn set_rewards(&mut self, rewards: u64) -> Result<(), NCNProgramError> {
        self.rewards = PodU64::from(rewards);
        Ok(())
    }

    /// Adds rewards to this route
    pub fn increment_rewards(&mut self, rewards: u64) -> Result<(), NCNProgramError> {
        let current_rewards = self.rewards();

        let new_rewards = current_rewards
            .checked_add(rewards)
            .ok_or(NCNProgramError::ArithmeticOverflow)?;

        self.set_rewards(new_rewards)
    }

    /// Removes rewards from this route (used during distribution)
    pub fn decrement_rewards(&mut self, rewards: u64) -> Result<(), NCNProgramError> {
        let current_rewards = self.rewards();

        let new_rewards = current_rewards
            .checked_sub(rewards)
            .ok_or(NCNProgramError::ArithmeticUnderflowError)?;

        self.set_rewards(new_rewards)
    }
}

/// Display implementation for OperatorVaultRewardRouter - provides formatted output for debugging
#[rustfmt::skip]
impl fmt::Display for OperatorVaultRewardRouter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "\n\n----------- Operator Vault Reward Route -------------")?;
        writeln!(f, "  Operator:                     {}", self.operator)?;
        writeln!(f, "  NCN:                          {}", self.ncn)?;
        writeln!(f, "  Epoch:                        {}", self.epoch())?;
        writeln!(f, "  Bump:                         {}", self.bump)?;
        writeln!(f, "  Slot Created:                 {}", self.slot_created())?;
        writeln!(f, "  NCN Operator Index:           {}", self.ncn_operator_index())?;
        writeln!(f, "  Still Routing:                {}", self.still_routing())?;
        writeln!(f, "  Total Rewards:                {}", self.total_rewards())?;
        writeln!(f, "  Reward Pool:                  {}", self.reward_pool())?;
        writeln!(f, "  Rewards Processed:            {}", self.rewards_processed())?;
        writeln!(f, "  Operator Rewards:             {}", self.operator_rewards())?;

        if self.still_routing() {
            writeln!(f, "\nRouting State:")?;
            writeln!(f, "  Last Rewards to Process:      {}", self.last_rewards_to_process())?;
            writeln!(f, "  Last Vault Op Del Index:      {}", self.last_vault_operator_delegation_index())?;
        }

        writeln!(f, "\nVault Reward Routes:")?;
        for route in self.vault_reward_routes().iter() {
            if !route.is_empty() {
                writeln!(f, "  Vault:                        {}", route.vault())?;
                if route.has_rewards() {
                    writeln!(f, "    Rewards:                    {}", route.rewards())?;
                }
            }
        }

        writeln!(f, "\n")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use solana_program::pubkey::Pubkey;

    use super::*;
    use crate::stake_weight::StakeWeights;

    const TEST_EPOCH: u64 = 1;
    const TEST_CURRENT_SLOT: u64 = 100;

    pub fn get_test_operator_snapshot(
        operator_fee_bps: u16,
        vault_operator_delegation_count: u64,
    ) -> OperatorSnapshot {
        let operator = Pubkey::new_unique();
        let ncn = Pubkey::new_unique();
        let epoch = TEST_EPOCH;
        let bump = 1;
        let current_slot = TEST_CURRENT_SLOT;
        let is_active = true;
        let ncn_operator_index = 0;
        let operator_index = 0;

        OperatorSnapshot::new(
            &operator,
            &ncn,
            epoch,
            bump,
            current_slot,
            is_active,
            ncn_operator_index,
            operator_index,
            operator_fee_bps,
            vault_operator_delegation_count,
        )
        .unwrap()
    }

    pub fn register_test_vault_operator_delegation(
        operator_snapshot: &mut OperatorSnapshot,
        stake_weight: u128,
    ) {
        let current_slot = TEST_CURRENT_SLOT;
        let vault = Pubkey::new_unique();
        let stake_weights = StakeWeights::snapshot(stake_weight).unwrap();

        let mut vault_index: u64 = 0;
        for index in 0..MAX_VAULTS {
            if !operator_snapshot.contains_vault_index(index as u64) {
                vault_index = index as u64;
                break;
            }
        }

        operator_snapshot
            .increment_vault_operator_delegation_registration(
                current_slot,
                &vault,
                vault_index,
                &stake_weights,
            )
            .unwrap()
    }

    #[test]
    fn test_len() {
        use std::mem::size_of;

        let expected_total = size_of::<Pubkey>() // operator
            + size_of::<Pubkey>() // ncn
            + size_of::<PodU64>() // epoch
            + 1 // bump
            + size_of::<PodU64>() // slot_created
            + size_of::<PodU64>() // operator_ncn_index
            + size_of::<PodU64>() // total_rewards
            + size_of::<PodU64>() // reward_pool
            + size_of::<PodU64>() // rewards_processed
            + size_of::<PodU64>() // operator_rewards
            + size_of::<PodU64>() // last_rewards_to_process
            + size_of::<PodU16>() // last_vault_operator_delegation_index
            + size_of::<VaultRewardRoute>() * MAX_VAULTS; // vault_reward_routes

        assert_eq!(size_of::<OperatorVaultRewardRouter>(), expected_total);
    }

    #[test]
    fn test_route_incoming_rewards() {
        let mut router = OperatorVaultRewardRouter::new(
            &Pubkey::new_unique(), // operator
            0,                     // operator_ncn_index
            &Pubkey::new_unique(), // ncn
            TEST_EPOCH,            // epoch
            1,                     // bump
            TEST_CURRENT_SLOT,     // slot_created
        );

        // Initial state checks
        assert_eq!(router.total_rewards(), 0);
        assert_eq!(router.reward_pool(), 0);
        assert_eq!(router.rewards_processed(), 0);

        // Test routing 1000 lamports
        let account_balance = 1000;
        router.route_incoming_rewards(0, account_balance).unwrap();

        // Verify rewards were routed correctly
        assert_eq!(router.total_rewards(), 1000);
        assert_eq!(router.reward_pool(), 1000);
        assert_eq!(router.rewards_processed(), 0);

        // Test routing additional 500 lamports
        let account_balance = 1500;
        router.route_incoming_rewards(0, account_balance).unwrap();

        // Verify total rewards increased by difference
        assert_eq!(router.total_rewards(), 1500);
        assert_eq!(router.reward_pool(), 1500);
        assert_eq!(router.rewards_processed(), 0);

        // Test attempting to route with lower balance (should fail)
        let result = router.route_incoming_rewards(0, 1000);
        assert!(result.is_err());

        // Verify state didn't change after failed routing
        assert_eq!(router.total_rewards(), 1500);
        assert_eq!(router.reward_pool(), 1500);
        assert_eq!(router.rewards_processed(), 0);
    }

    #[test]
    fn test_route_operator_rewards() {
        const INCOMING_REWARDS: u64 = 1000;

        let mut router = OperatorVaultRewardRouter::new(
            &Pubkey::new_unique(), // operator
            0,                     // operator_ncn_index
            &Pubkey::new_unique(), // ncn
            TEST_EPOCH,            // epoch
            1,                     // bump
            TEST_CURRENT_SLOT,     // slot_created
        );

        // Initial state checks
        assert_eq!(router.total_rewards(), 0);
        assert_eq!(router.reward_pool(), 0);
        assert_eq!(router.rewards_processed(), 0);

        // Test routing 1000 lamports
        router.route_incoming_rewards(0, INCOMING_REWARDS).unwrap();

        // Verify rewards were routed correctly
        assert_eq!(router.total_rewards(), INCOMING_REWARDS);
        assert_eq!(router.reward_pool(), INCOMING_REWARDS);
        assert_eq!(router.rewards_processed(), 0);

        let operator_snapshot = {
            let operator_fee_bps = 1000; // 10%
            let vault_operator_delegation_count = 10;
            let mut operator_snapshot =
                get_test_operator_snapshot(operator_fee_bps, vault_operator_delegation_count);

            for _ in 0..vault_operator_delegation_count {
                register_test_vault_operator_delegation(&mut operator_snapshot, 1000);
            }

            operator_snapshot
        };

        // Test routing operator rewards
        router.route_operator_rewards(&operator_snapshot).unwrap();
        assert_eq!(router.operator_rewards(), INCOMING_REWARDS / 10);

        assert_eq!(router.total_rewards(), INCOMING_REWARDS);
        assert_eq!(router.reward_pool(), INCOMING_REWARDS / 10 * 9);
        assert_eq!(router.rewards_processed(), INCOMING_REWARDS / 10);
    }

    #[test]
    fn test_max_iterations() {
        const INCOMING_REWARDS: u64 = 1000;

        let mut router = OperatorVaultRewardRouter::new(
            &Pubkey::new_unique(), // operator
            0,                     // operator_ncn_index
            &Pubkey::new_unique(), // ncn
            TEST_EPOCH,            // epoch
            1,                     // bump
            TEST_CURRENT_SLOT,     // slot_created
        );

        // Initial state checks
        assert_eq!(router.total_rewards(), 0);
        assert_eq!(router.reward_pool(), 0);
        assert_eq!(router.rewards_processed(), 0);

        // Test routing 1000 lamports
        router.route_incoming_rewards(0, INCOMING_REWARDS).unwrap();

        // Verify rewards were routed correctly
        assert_eq!(router.total_rewards(), INCOMING_REWARDS);
        assert_eq!(router.reward_pool(), INCOMING_REWARDS);
        assert_eq!(router.rewards_processed(), 0);

        let operator_snapshot = {
            let operator_fee_bps = 0; // 0%
            let vault_operator_delegation_count = 10;
            let mut operator_snapshot =
                get_test_operator_snapshot(operator_fee_bps, vault_operator_delegation_count);

            for _ in 0..vault_operator_delegation_count {
                register_test_vault_operator_delegation(&mut operator_snapshot, 1000);
            }

            operator_snapshot
        };

        // Test routing operator rewards
        router.route_operator_rewards(&operator_snapshot).unwrap();
        assert_eq!(router.operator_rewards(), 0);

        assert_eq!(router.total_rewards(), INCOMING_REWARDS);
        assert_eq!(router.reward_pool(), INCOMING_REWARDS);
        assert_eq!(router.rewards_processed(), 0);

        router.route_reward_pool(&operator_snapshot, 5).unwrap();

        assert_eq!(router.still_routing(), true);

        router.route_reward_pool(&operator_snapshot, 1000).unwrap();

        assert_eq!(router.still_routing(), false);

        for route in router
            .vault_reward_routes()
            .iter()
            .filter(|route| !route.is_empty())
        {
            assert_eq!(route.rewards(), 100);
        }

        assert_eq!(router.total_rewards(), INCOMING_REWARDS);
        assert_eq!(router.reward_pool(), 0);
        assert_eq!(router.rewards_processed(), INCOMING_REWARDS);
    }

    #[test]
    fn test_route_max_vaults() {
        const INCOMING_REWARDS: u64 = MAX_VAULTS as u64 * 1000;

        let mut router = OperatorVaultRewardRouter::new(
            &Pubkey::new_unique(), // operator
            0,                     // operator_ncn_index
            &Pubkey::new_unique(), // ncn
            TEST_EPOCH,            // epoch
            1,                     // bump
            TEST_CURRENT_SLOT,     // slot_created
        );

        // Initial state checks
        assert_eq!(router.total_rewards(), 0);
        assert_eq!(router.reward_pool(), 0);
        assert_eq!(router.rewards_processed(), 0);

        // Test routing 1000 lamports
        router.route_incoming_rewards(0, INCOMING_REWARDS).unwrap();

        // Verify rewards were routed correctly
        assert_eq!(router.total_rewards(), INCOMING_REWARDS);
        assert_eq!(router.reward_pool(), INCOMING_REWARDS);
        assert_eq!(router.rewards_processed(), 0);

        let operator_snapshot = {
            let operator_fee_bps = 0; // 0%
            let vault_operator_delegation_count = MAX_VAULTS as u64;
            let mut operator_snapshot =
                get_test_operator_snapshot(operator_fee_bps, vault_operator_delegation_count);

            for _ in 0..vault_operator_delegation_count {
                register_test_vault_operator_delegation(&mut operator_snapshot, 1000);
            }

            operator_snapshot
        };

        // Test routing operator rewards
        router.route_operator_rewards(&operator_snapshot).unwrap();
        assert_eq!(router.operator_rewards(), 0);

        assert_eq!(router.total_rewards(), INCOMING_REWARDS);
        assert_eq!(router.reward_pool(), INCOMING_REWARDS);
        assert_eq!(router.rewards_processed(), 0);

        router.route_reward_pool(&operator_snapshot, 1000).unwrap();
        for route in router
            .vault_reward_routes()
            .iter()
            .filter(|route| !route.is_empty())
        {
            assert_eq!(route.rewards(), 1000);
        }

        assert_eq!(router.total_rewards(), INCOMING_REWARDS);
        assert_eq!(router.reward_pool(), 0);
        assert_eq!(router.rewards_processed(), INCOMING_REWARDS);
    }

    #[test]
    fn test_route_max_vaults_with_operator() {
        let expected_vault_rewards: u64 = 1000;
        let expected_all_vault_rewards: u64 = MAX_VAULTS as u64 * expected_vault_rewards;
        let incoming_rewards: u64 = (expected_all_vault_rewards as f64 / 0.9).round() as u64;
        let expected_operator_rewards: u64 = incoming_rewards - expected_all_vault_rewards;

        let mut router = OperatorVaultRewardRouter::new(
            &Pubkey::new_unique(), // operator
            0,                     // operator_ncn_index
            &Pubkey::new_unique(), // ncn
            TEST_EPOCH,            // epoch
            1,                     // bump
            TEST_CURRENT_SLOT,     // slot_created
        );

        // Initial state checks
        assert_eq!(router.total_rewards(), 0);
        assert_eq!(router.reward_pool(), 0);
        assert_eq!(router.rewards_processed(), 0);

        // Test routing 1000 lamports
        router.route_incoming_rewards(0, incoming_rewards).unwrap();

        // Verify rewards were routed correctly
        assert_eq!(router.total_rewards(), incoming_rewards);
        assert_eq!(router.reward_pool(), incoming_rewards);
        assert_eq!(router.rewards_processed(), 0);

        let operator_snapshot = {
            let operator_fee_bps = 1000; // 10%
            let vault_operator_delegation_count = MAX_VAULTS as u64;
            let mut operator_snapshot =
                get_test_operator_snapshot(operator_fee_bps, vault_operator_delegation_count);

            for _ in 0..vault_operator_delegation_count {
                register_test_vault_operator_delegation(&mut operator_snapshot, 1000);
            }

            operator_snapshot
        };

        // Test routing operator rewards
        router.route_operator_rewards(&operator_snapshot).unwrap();
        assert_eq!(router.operator_rewards(), expected_operator_rewards);

        assert_eq!(router.total_rewards(), incoming_rewards);
        assert_eq!(router.reward_pool(), expected_all_vault_rewards);
        assert_eq!(router.rewards_processed(), expected_operator_rewards);

        router.route_reward_pool(&operator_snapshot, 1000).unwrap();
        for route in router
            .vault_reward_routes()
            .iter()
            .filter(|route| !route.is_empty())
        {
            assert_eq!(route.rewards(), expected_vault_rewards);
        }

        assert_eq!(router.total_rewards(), incoming_rewards);
        assert_eq!(router.reward_pool(), 0);
        assert_eq!(router.rewards_processed(), incoming_rewards);
    }

    #[test]
    fn test_route_with_0_iterations() {
        let expected_vault_rewards: u64 = 1000;
        let expected_all_vault_rewards: u64 = MAX_VAULTS as u64 * expected_vault_rewards;
        let incoming_rewards: u64 = (expected_all_vault_rewards as f64 / 0.9).round() as u64;
        let expected_operator_rewards: u64 = incoming_rewards - expected_all_vault_rewards;

        let mut router = OperatorVaultRewardRouter::new(
            &Pubkey::new_unique(), // operator
            0,                     // operator_ncn_index
            &Pubkey::new_unique(), // ncn
            TEST_EPOCH,            // epoch
            1,                     // bump
            TEST_CURRENT_SLOT,     // slot_created
        );

        // Initial state checks
        assert_eq!(router.total_rewards(), 0);
        assert_eq!(router.reward_pool(), 0);
        assert_eq!(router.rewards_processed(), 0);

        // Test routing 1000 lamports
        router.route_incoming_rewards(0, incoming_rewards).unwrap();

        // Verify rewards were routed correctly
        assert_eq!(router.total_rewards(), incoming_rewards);
        assert_eq!(router.reward_pool(), incoming_rewards);
        assert_eq!(router.rewards_processed(), 0);

        let operator_snapshot = {
            let operator_fee_bps = 1000; // 10%
            let vault_operator_delegation_count = MAX_VAULTS as u64;
            let mut operator_snapshot =
                get_test_operator_snapshot(operator_fee_bps, vault_operator_delegation_count);

            for _ in 0..vault_operator_delegation_count {
                register_test_vault_operator_delegation(&mut operator_snapshot, 1000);
            }

            operator_snapshot
        };

        // Test routing operator rewards
        router.route_operator_rewards(&operator_snapshot).unwrap();
        assert_eq!(router.operator_rewards(), expected_operator_rewards);

        assert_eq!(router.total_rewards(), incoming_rewards);
        assert_eq!(router.reward_pool(), expected_all_vault_rewards);
        assert_eq!(router.rewards_processed(), expected_operator_rewards);

        router.route_reward_pool(&operator_snapshot, 0).unwrap();
        assert!(router.still_routing());

        for _ in 0..MAX_VAULTS * 2 {
            router.route_reward_pool(&operator_snapshot, 0).unwrap();
        }
        assert!(!router.still_routing());

        for route in router
            .vault_reward_routes()
            .iter()
            .filter(|route| !route.is_empty())
        {
            assert_eq!(route.rewards(), expected_vault_rewards);
        }

        assert_eq!(router.total_rewards(), incoming_rewards);
        assert_eq!(router.reward_pool(), 0);
        assert_eq!(router.rewards_processed(), incoming_rewards);
    }
}
