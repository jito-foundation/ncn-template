use core::{fmt, mem::size_of};

use bytemuck::{Pod, Zeroable};
use jito_bytemuck::{
    types::{PodU16, PodU64},
    AccountDeserialize, Discriminator,
};
use shank::{ShankAccount, ShankType};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, msg, program::invoke_signed,
    program_error::ProgramError, pubkey::Pubkey, rent::Rent, system_instruction, system_program,
    sysvar::Sysvar,
};
use spl_math::precise_number::PreciseNumber;

use crate::{
    ballot_box::BallotBox, discriminators::Discriminators, error::NCNProgramError, fees::Fees,
    loaders::check_load,
};

/// NCN Reward Router - Main entry point for routing rewards from NCNs
///
/// This router receives rewards and distributes them according to the fee structure:
/// 1. Jito DAO receives a percentage (4%)
/// 2. NCN receives a percentage (4%)
/// 3. Remaining rewards (92%) go to operator-vault rewards
///
/// The router supports partial routing through iterations to handle large numbers of operators
/// without hitting transaction limits.
///
/// PDA: ["ncn_reward_router", NCN, NCN_EPOCH_SLOT]
#[derive(Debug, Clone, Copy, Zeroable, Pod, AccountDeserialize, ShankAccount)]
#[repr(C)]
pub struct NCNRewardRouter {
    /// NCN the account is associated with
    ncn: Pubkey,
    /// The epoch the account is associated with
    epoch: PodU64,
    /// Bump seed for the PDA
    bump: u8,
    /// Slot the account was created
    slot_created: PodU64,
    /// Total rewards routed (in lamports) - cumulative amount ever processed
    total_rewards: PodU64,
    /// Amount of rewards in the reward pool (in lamports) - awaiting distribution
    reward_pool: PodU64,
    /// Amount of rewards processed (in lamports) - moved out of reward pool for distribution
    rewards_processed: PodU64,
    /// Reserved space for future fields
    reserved: [u8; 128],

    // Routing state tracking - enables recovery from incomplete routing operations
    /// Last vote index processed during routing (for resuming partial operations)
    last_vote_index: PodU16,
    /// Last rewards amount being processed during routing (for resuming partial operations)
    last_rewards_to_process: PodU64,

    /// Rewards allocated to the Jito DAO (ready for distribution)
    jito_dao_rewards: PodU64,
    /// Rewards allocated to the NCN (ready for distribution)
    ncn_rewards: PodU64,

    /// Total rewards allocated to operator-vault reward receivers (before individual routing)
    operator_vault_rewards: PodU64,

    /// Individual operator reward routes - tracks rewards per operator
    /// Array size 256 limits the number of operators that can participate in an epoch
    operator_vault_reward_routes: [OperatorVaultRewardRoute; 256],
}

impl Discriminator for NCNRewardRouter {
    const DISCRIMINATOR: u8 = Discriminators::NCNRewardRouter as u8;
}

impl NCNRewardRouter {
    pub const SIZE: usize = 8 + size_of::<Self>();
    pub const NCN_REWARD_ROUTER_SEED: &'static [u8] = b"ncn_reward_router";

    /// Fee constants - in basis points (1/100 of 1%)
    pub const JITO_DAO_FEE_BPS: u16 = 400; // 4%
    pub const NCN_DEFAULT_FEE_BPS: u16 = 400; // 4%

    /// Sentinel values indicating no partial routing is in progress
    pub const NO_LAST_NCN_GROUP_INDEX: u8 = u8::MAX;
    pub const NO_LAST_VOTE_INDEX: u16 = u16::MAX;
    pub const NO_LAST_REWARDS_TO_PROCESS: u64 = u64::MAX;

    /// Maximum iterations per routing call to prevent transaction timeout
    pub const MAX_ROUTE_BASE_ITERATIONS: u16 = 30;

    /// Creates a new NCN reward router
    pub fn new(ncn: &Pubkey, ncn_epoch: u64, bump: u8, slot_created: u64) -> Self {
        Self {
            ncn: *ncn,
            epoch: PodU64::from(ncn_epoch),
            bump,
            slot_created: PodU64::from(slot_created),
            total_rewards: PodU64::from(0),
            reward_pool: PodU64::from(0),
            rewards_processed: PodU64::from(0),
            reserved: [0; 128],
            last_vote_index: PodU16::from(Self::NO_LAST_VOTE_INDEX),
            last_rewards_to_process: PodU64::from(Self::NO_LAST_REWARDS_TO_PROCESS),
            jito_dao_rewards: PodU64::from(0),
            ncn_rewards: PodU64::from(0),
            operator_vault_rewards: PodU64::from(0),
            operator_vault_reward_routes: [OperatorVaultRewardRoute::default(); 256],
        }
    }

    /// Initializes the router fields individually to avoid stack overflow
    /// This approach is necessary due to the large size of the struct
    pub fn initialize(&mut self, ncn: &Pubkey, ncn_epoch: u64, bump: u8, current_slot: u64) {
        // Initializes field by field to avoid overflowing stack
        self.ncn = *ncn;
        self.epoch = PodU64::from(ncn_epoch);
        self.bump = bump;
        self.slot_created = PodU64::from(current_slot);
        self.total_rewards = PodU64::from(0);
        self.reward_pool = PodU64::from(0);
        self.rewards_processed = PodU64::from(0);
        self.reserved = [0; 128];
        self.jito_dao_rewards = PodU64::from(0);
        self.ncn_rewards = PodU64::from(0);
        self.operator_vault_rewards = PodU64::from(0);
        self.operator_vault_reward_routes = [OperatorVaultRewardRoute::default(); 256];

        self.reset_routing_state();
    }

    /// Generates PDA seeds for the NCN reward router
    pub fn seeds(ncn: &Pubkey, ncn_epoch: u64) -> Vec<Vec<u8>> {
        Vec::from_iter(
            [
                Self::NCN_REWARD_ROUTER_SEED.to_vec(),
                ncn.to_bytes().to_vec(),
                ncn_epoch.to_le_bytes().to_vec(),
            ]
            .iter()
            .cloned(),
        )
    }

    /// Finds the program address for the NCN reward router PDA
    pub fn find_program_address(
        program_id: &Pubkey,
        ncn: &Pubkey,
        epoch: u64,
    ) -> (Pubkey, u8, Vec<Vec<u8>>) {
        let seeds: Vec<Vec<u8>> = Self::seeds(ncn, epoch);
        let seeds_iter: Vec<_> = seeds.iter().map(|s| s.as_slice()).collect();
        let (pda, bump) = Pubkey::find_program_address(&seeds_iter, program_id);
        (pda, bump, seeds)
    }

    /// Validates that the account matches expected PDA and discriminator
    pub fn load(
        program_id: &Pubkey,
        account: &AccountInfo,
        ncn: &Pubkey,
        epoch: u64,
        expect_writable: bool,
    ) -> Result<(), ProgramError> {
        let expected_pda = Self::find_program_address(program_id, ncn, epoch).0;
        check_load(
            program_id,
            account,
            &expected_pda,
            Some(Self::DISCRIMINATOR),
            expect_writable,
        )
    }

    /// Loads the account for closing (must be writable)
    pub fn load_to_close(
        program_id: &Pubkey,
        account_to_close: &AccountInfo,
        ncn: &Pubkey,
        epoch: u64,
    ) -> Result<(), ProgramError> {
        Self::load(program_id, account_to_close, ncn, epoch, true)
    }

    // ----------------- ROUTE STATE TRACKING --------------

    /// Gets the last vote index processed during partial routing
    pub fn last_vote_index(&self) -> u16 {
        self.last_vote_index.into()
    }

    /// Gets the NCN rewards ready for distribution
    pub fn ncn_rewards(&self) -> u64 {
        self.ncn_rewards.into()
    }

    /// Gets the total operator vault rewards ready for distribution
    pub fn operator_vault_rewards(&self) -> u64 {
        self.operator_vault_rewards.into()
    }

    /// Gets the operator vault reward routes array
    pub fn operator_vault_reward_routes(&self) -> &[OperatorVaultRewardRoute; 256] {
        &self.operator_vault_reward_routes
    }

    /// Gets the last rewards amount being processed during partial routing
    pub fn last_rewards_to_process(&self) -> u64 {
        self.last_rewards_to_process.into()
    }

    /// Resumes routing from the last saved state if routing was interrupted
    /// Returns (starting_vote_index, rewards_to_process)
    pub fn resume_routing_state(&mut self) -> (usize, u64) {
        if !self.still_routing() {
            return (0, 0);
        }

        (
            self.last_vote_index() as usize,
            self.last_rewards_to_process(),
        )
    }

    /// Saves the current routing state for resumption if interrupted
    pub fn save_routing_state(&mut self, vote_index: usize, rewards_to_process: u64) {
        self.last_vote_index = PodU16::from(vote_index as u16);
        self.last_rewards_to_process = PodU64::from(rewards_to_process);
    }

    /// Resets routing state to indicate no partial routing is in progress
    pub fn reset_routing_state(&mut self) {
        self.last_vote_index = PodU16::from(Self::NO_LAST_VOTE_INDEX);
        self.last_rewards_to_process = PodU64::from(Self::NO_LAST_REWARDS_TO_PROCESS);
    }

    /// Checks if routing is still in progress (was interrupted)
    pub fn still_routing(&self) -> bool {
        self.last_vote_index() != Self::NO_LAST_VOTE_INDEX
            || self.last_rewards_to_process() != Self::NO_LAST_REWARDS_TO_PROCESS
    }

    // ----------------- ROUTE REWARDS ---------------------

    /// Routes incoming rewards from account balance to the reward pool
    /// This is the entry point for new rewards coming into the router
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

    /// Routes rewards from the reward pool to Jito DAO and NCN based on fee structure
    /// This is the first phase of reward distribution
    pub fn route_reward_pool(&mut self, fee: &Fees) -> Result<(), NCNProgramError> {
        let rewards_to_process: u64 = self.reward_pool();

        // Route Jito DAO fee (typically 4%)
        {
            let jito_dao_fee =
                Self::calculate_reward_split(fee.jito_dao_fee_bps()?, rewards_to_process)?;
            self.route_from_reward_pool(jito_dao_fee)?;
            self.route_to_jito_dao(jito_dao_fee)?;
        }

        // Route NCN fee (typically 4%)
        {
            let ncn_fee = Self::calculate_reward_split(fee.ncn_fee_bps()?, rewards_to_process)?;
            self.route_from_reward_pool(ncn_fee)?;
            self.route_to_ncn(ncn_fee)?;
        }

        // The rest goes to operator-vault rewards (typically 92%)
        {
            let operator_vault_rewards = self.reward_pool();
            self.route_from_reward_pool(operator_vault_rewards)?;
            self.route_to_operator_vault(operator_vault_rewards)?;
        }

        Ok(())
    }

    /// Routes operator vault rewards to individual operators based on their vote participation
    /// This is the second phase of reward distribution that can be done iteratively
    pub fn route_operator_vault_rewards(
        &mut self,
        ballot_box: &BallotBox,
        max_iterations: u16,
    ) -> Result<(), NCNProgramError> {
        let winning_ballot = ballot_box.get_winning_ballot_tally()?;
        let winning_stake_weight = winning_ballot.stake_weights();

        let (starting_vote_index, starting_rewards_to_process) = self.resume_routing_state();

        let mut iterations: u16 = 0;
        // Always have at least 1 iteration to make progress
        let max_iterations = max_iterations.max(1);

        let rewards_to_process = if starting_rewards_to_process > 0 {
            starting_rewards_to_process
        } else {
            self.operator_vault_rewards()
        };

        if rewards_to_process == 0 {
            return Ok(());
        }

        // Iterate through operator votes and distribute rewards to winning voters
        for vote_index in starting_vote_index..ballot_box.operator_votes().len() {
            let vote = ballot_box.operator_votes()[vote_index];

            // Only reward operators who voted for the winning ballot
            if vote.ballot_index() == winning_ballot.index() {
                // Track iterations to prevent transaction timeout
                {
                    iterations = iterations
                        .checked_add(1)
                        .ok_or(NCNProgramError::ArithmeticOverflow)?;

                    if iterations > max_iterations {
                        msg!(
                            "Reached max iterations, saving state and exiting {}",
                            vote_index
                        );
                        self.save_routing_state(vote_index, rewards_to_process);
                        return Ok(());
                    }
                }

                let operator = vote.operator();

                let winning_reward_stake_weight = winning_stake_weight.stake_weight();
                let operator_vote_stake_weight = vote.stake_weights().stake_weight();

                // Calculate proportional reward based on operator's stake weight
                let operator_route_reward = Self::calculate_operator_vault_route_reward(
                    operator_vote_stake_weight,
                    winning_reward_stake_weight,
                    rewards_to_process,
                )?;

                self.route_from_operator_vault_rewards(operator_route_reward)?;
                self.route_to_operator_vault_reward_route(operator, operator_route_reward)?;
            }
        }

        // NCN gets any remaining rewards due to rounding
        {
            let leftover_rewards = self.operator_vault_rewards();

            self.route_from_operator_vault_rewards(leftover_rewards)?;
            self.route_to_ncn(leftover_rewards)?;
        }

        msg!("Finished routing operator vault rewards");
        self.reset_routing_state();

        Ok(())
    }

    // ------------------ CALCULATIONS ---------------------

    /// Calculates reward amount based on basis points
    /// Used for fee calculations (Jito DAO and NCN fees)
    fn calculate_reward_split(
        fee_basis_points: u16,
        total_rewards: u64,
    ) -> Result<u64, NCNProgramError> {
        const BASIS_POINTS_DENOMINATOR: u16 = 10000;

        if fee_basis_points == 0 || total_rewards == 0 {
            return Ok(0);
        }

        let precise_fee_basis_points = PreciseNumber::new(fee_basis_points as u128)
            .ok_or(NCNProgramError::NewPreciseNumberError)?;

        let precise_basis_points_denominator = PreciseNumber::new(BASIS_POINTS_DENOMINATOR as u128)
            .ok_or(NCNProgramError::NewPreciseNumberError)?;

        let precise_total_rewards = PreciseNumber::new(total_rewards as u128)
            .ok_or(NCNProgramError::NewPreciseNumberError)?;

        let precise_fee_amount = precise_total_rewards
            .checked_mul(&precise_fee_basis_points)
            .and_then(|x| x.checked_div(&precise_basis_points_denominator))
            .ok_or(NCNProgramError::ArithmeticOverflow)?;

        let floored_precise_fee_amount = precise_fee_amount
            .floor()
            .ok_or(NCNProgramError::ArithmeticFloorError)?;

        let fee_amount_u128: u128 = floored_precise_fee_amount
            .to_imprecise()
            .ok_or(NCNProgramError::CastToImpreciseNumberError)?;
        let fee_amount: u64 = fee_amount_u128
            .try_into()
            .map_err(|_| NCNProgramError::CastToU64Error)?;

        Ok(fee_amount)
    }

    /// Calculates proportional reward for an operator based on their stake weight
    /// Formula: (operator_stake_weight / total_winning_stake_weight) * total_rewards
    fn calculate_operator_vault_route_reward(
        operator_stake_weight: u128,
        winning_total_stake_weight: u128,
        rewards_to_process: u64,
    ) -> Result<u64, NCNProgramError> {
        if operator_stake_weight == 0 || rewards_to_process == 0 {
            return Ok(0);
        }

        let precise_rewards_to_process = PreciseNumber::new(rewards_to_process as u128)
            .ok_or(NCNProgramError::NewPreciseNumberError)?;

        let precise_operator_stake_weight = PreciseNumber::new(operator_stake_weight)
            .ok_or(NCNProgramError::NewPreciseNumberError)?;

        let precise_winning_total_stake_weight = PreciseNumber::new(winning_total_stake_weight)
            .ok_or(NCNProgramError::NewPreciseNumberError)?;

        let precise_operator_reward = precise_rewards_to_process
            .checked_mul(&precise_operator_stake_weight)
            .and_then(|x| x.checked_div(&precise_winning_total_stake_weight))
            .ok_or(NCNProgramError::ArithmeticOverflow)?;

        let floored_precise_operator_reward = precise_operator_reward
            .floor()
            .ok_or(NCNProgramError::ArithmeticFloorError)?;

        let operator_reward_u128: u128 = floored_precise_operator_reward
            .to_imprecise()
            .ok_or(NCNProgramError::CastToImpreciseNumberError)?;

        let operator_reward: u64 = operator_reward_u128
            .try_into()
            .map_err(|_| NCNProgramError::CastToU64Error)?;

        Ok(operator_reward)
    }

    // ------------------ REWARD TALLIES ---------------------

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

    pub const fn ncn(&self) -> &Pubkey {
        &self.ncn
    }

    pub fn epoch(&self) -> u64 {
        self.epoch.into()
    }

    pub fn slot_created(&self) -> u64 {
        self.slot_created.into()
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

    /// Moves rewards out of operator vault rewards pool
    pub fn route_from_operator_vault_rewards(
        &mut self,
        rewards: u64,
    ) -> Result<(), NCNProgramError> {
        if rewards == 0 {
            return Ok(());
        }

        self.operator_vault_rewards = PodU64::from(
            self.operator_vault_rewards()
                .checked_sub(rewards)
                .ok_or(NCNProgramError::ArithmeticUnderflowError)?,
        );

        Ok(())
    }

    /// Routes rewards to Jito DAO allocation
    pub fn route_to_jito_dao(&mut self, rewards: u64) -> Result<(), NCNProgramError> {
        if rewards == 0 {
            return Ok(());
        }

        self.jito_dao_rewards = PodU64::from(
            self.jito_dao_rewards()
                .checked_add(rewards)
                .ok_or(NCNProgramError::ArithmeticOverflow)?,
        );

        Ok(())
    }

    /// Routes rewards to NCN allocation
    pub fn route_to_ncn(&mut self, rewards: u64) -> Result<(), NCNProgramError> {
        if rewards == 0 {
            return Ok(());
        }

        self.ncn_rewards = PodU64::from(
            self.ncn_rewards()
                .checked_add(rewards)
                .ok_or(NCNProgramError::ArithmeticOverflow)?,
        );

        Ok(())
    }

    /// Routes rewards to operator vault allocation
    pub fn route_to_operator_vault(&mut self, rewards: u64) -> Result<(), NCNProgramError> {
        if rewards == 0 {
            return Ok(());
        }

        self.operator_vault_rewards = PodU64::from(
            self.operator_vault_rewards()
                .checked_add(rewards)
                .ok_or(NCNProgramError::ArithmeticOverflow)?,
        );

        Ok(())
    }

    // ------------------ REWARDS PROCESSED ---------------------

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

    pub fn jito_dao_rewards(&self) -> u64 {
        self.jito_dao_rewards.into()
    }

    /// Distributes Jito DAO rewards and updates counters
    /// Returns the amount of rewards distributed
    pub fn distribute_base_fee_group_rewards(&mut self) -> Result<u64, NCNProgramError> {
        let rewards = self.jito_dao_rewards();
        self.jito_dao_rewards = PodU64::from(
            rewards
                .checked_sub(rewards)
                .ok_or(NCNProgramError::ArithmeticUnderflowError)?,
        );

        self.decrement_rewards_processed(rewards)?;

        Ok(rewards)
    }

    // ------------------ OPERATOR VAULT REWARD ROUTES ---------------------

    /// Checks if an operator has a reward route
    pub fn has_operator_vault_reward_route(&self, operator: &Pubkey) -> bool {
        for operator_vault_route_reward in self.operator_vault_reward_routes().iter() {
            if operator_vault_route_reward.operator.eq(operator) {
                return true;
            }
        }

        false
    }

    /// Gets the reward route for a specific operator
    pub fn oprtator_vault_reward_route(
        &self,
        operator: &Pubkey,
    ) -> Result<&OperatorVaultRewardRoute, NCNProgramError> {
        for operator_vault_route_reward in self.operator_vault_reward_routes().iter() {
            if operator_vault_route_reward.operator.eq(operator) {
                return Ok(operator_vault_route_reward);
            }
        }

        Err(NCNProgramError::NcnRewardRouteNotFound)
    }

    /// Routes rewards to a specific operator's reward route
    /// Creates a new route if one doesn't exist for the operator
    pub fn route_to_operator_vault_reward_route(
        &mut self,
        operator: &Pubkey,
        rewards: u64,
    ) -> Result<(), NCNProgramError> {
        if rewards == 0 {
            return Ok(());
        }

        // Try to find existing route and increment rewards
        for operator_vault_route_reward in self.operator_vault_reward_routes.iter_mut() {
            if operator_vault_route_reward.operator.eq(operator) {
                operator_vault_route_reward.increment_rewards(rewards)?;
                return Ok(());
            } else if operator_vault_route_reward.operator.eq(&Pubkey::default()) {
                // Found empty slot, create new route
                *operator_vault_route_reward = OperatorVaultRewardRoute::new(operator, rewards)?;
                return Ok(());
            }
        }

        Err(NCNProgramError::OperatorRewardListFull)
    }

    /// Distributes rewards for a specific operator and updates counters
    /// Returns the amount of rewards distributed
    pub fn distribute_operator_vault_reward_route(
        &mut self,
        operator: &Pubkey,
    ) -> Result<u64, NCNProgramError> {
        for route in self.operator_vault_reward_routes.iter_mut() {
            if route.operator.eq(operator) {
                let rewards = route.rewards()?;
                route.decrement_rewards(rewards)?;
                self.decrement_rewards_processed(rewards)?;

                return Ok(rewards);
            }
        }

        Err(NCNProgramError::OperatorRewardNotFound)
    }
}

#[rustfmt::skip]
impl fmt::Display for NCNRewardRouter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "\n\n----------- NCN Reward Router -------------")?;
        writeln!(f, "  NCN:                          {}", self.ncn)?;
        writeln!(f, "  Epoch:                        {}", self.epoch())?;
        writeln!(f, "  Bump:                         {}", self.bump)?;
        writeln!(f, "  Slot Created:                 {}", self.slot_created())?;
        writeln!(f, "  Still Routing:                {}", self.still_routing())?;
        writeln!(f, "  Total Rewards:                {}", self.total_rewards())?;
        writeln!(f, "  Reward Pool:                  {}", self.reward_pool())?;
        writeln!(f, "  Rewards Processed:            {}", self.rewards_processed())?;

        if self.still_routing() {
            writeln!(f, "\nRouting State:")?;
            writeln!(f, "  Last Vote Index:              {}", self.last_vote_index())?;
            writeln!(f, "  Last Rewards to Process:      {}", self.last_rewards_to_process())?;
        }

        writeln!(f, "\nRewards:")?;
        writeln!(f, "  Jito DAO Rewards:             {}", self.jito_dao_rewards())?;
        writeln!(f, "  NCN Rewards:                  {}", self.ncn_rewards())?;
        writeln!(
            f,
            "  Operator Vault Rewards:       {}",
            self.operator_vault_rewards()
        )?;

        writeln!(f, "\nOperator Vault Reward Routes:")?;
        for route in self.operator_vault_reward_routes().iter() {
            if !route.is_empty() {
                writeln!(f, "  Operator:                     {}", route.operator())?;
                if let Ok(rewards) = route.rewards() {
                    if rewards > 0 {
                        writeln!(f, "    Rewards:                      {}", rewards)?;
                    }
                }
            }
        }

        writeln!(f, "\n")?;
        Ok(())
    }
}

/// Individual operator reward route - tracks rewards for a specific operator
/// This struct stores the allocation of rewards for an operator before distribution
#[derive(Debug, Clone, PartialEq, Eq, Copy, Zeroable, ShankType, Pod)]
#[repr(C)]
pub struct OperatorVaultRewardRoute {
    /// The operator pubkey
    operator: Pubkey,
    /// Reward amount allocated to this operator
    rewards: NCNRewardRouterRewards,
}

impl Default for OperatorVaultRewardRoute {
    fn default() -> Self {
        Self {
            operator: Pubkey::default(),
            rewards: NCNRewardRouterRewards::default(),
        }
    }
}

impl OperatorVaultRewardRoute {
    /// Creates a new operator vault reward route with initial reward amount
    pub fn new(operator: &Pubkey, rewards: u64) -> Result<Self, NCNProgramError> {
        let mut route = Self {
            operator: *operator,
            rewards: NCNRewardRouterRewards::default(),
        };

        route.set_rewards(rewards)?;

        Ok(route)
    }

    /// Gets the operator pubkey for this route
    pub const fn operator(&self) -> &Pubkey {
        &self.operator
    }

    /// Gets the reward amount for this route
    pub fn rewards(&self) -> Result<u64, NCNProgramError> {
        Ok(self.rewards.rewards())
    }

    /// Checks if this route slot is empty (default operator)
    pub fn is_empty(&self) -> bool {
        self.operator.eq(&Pubkey::default())
    }

    /// Checks if this route has any rewards allocated
    pub fn has_rewards(&self) -> Result<bool, NCNProgramError> {
        if self.rewards()? > 0 {
            return Ok(true);
        }

        Ok(false)
    }

    /// Sets the reward amount for this route
    fn set_rewards(&mut self, rewards: u64) -> Result<(), NCNProgramError> {
        self.rewards.rewards = PodU64::from(rewards);

        Ok(())
    }

    /// Adds rewards to this route
    pub fn increment_rewards(&mut self, rewards: u64) -> Result<(), NCNProgramError> {
        let current_rewards = self.rewards()?;

        let new_rewards = current_rewards
            .checked_add(rewards)
            .ok_or(NCNProgramError::ArithmeticOverflow)?;

        self.set_rewards(new_rewards)
    }

    /// Removes rewards from this route (used during distribution)
    pub fn decrement_rewards(&mut self, rewards: u64) -> Result<(), NCNProgramError> {
        let current_rewards = self.rewards()?;

        let new_rewards = current_rewards
            .checked_sub(rewards)
            .ok_or(NCNProgramError::ArithmeticOverflow)?;

        self.set_rewards(new_rewards)
    }
}

/// NCN Reward Receiver - Uninitialized account that receives rewards for an NCN
///
/// This is a simple PDA account with no data that acts as a destination for rewards.
/// It must remain uninitialized to be used as a payer in transfer instructions.
/// The account is closed after rewards are transferred to the NCN Reward Router.
///
/// PDA: ["ncn_reward_receiver", NCN, EPOCH]
pub struct NCNRewardReceiver {}

impl NCNRewardReceiver {
    /// Generates PDA seeds for the NCN reward receiver
    pub fn seeds(ncn: &Pubkey, epoch: u64) -> Vec<Vec<u8>> {
        vec![
            b"ncn_reward_receiver".to_vec(),
            ncn.to_bytes().to_vec(),
            epoch.to_le_bytes().to_vec(),
        ]
    }

    /// Finds the program address for the NCN reward receiver PDA
    pub fn find_program_address(
        program_id: &Pubkey,
        ncn: &Pubkey,
        epoch: u64,
    ) -> (Pubkey, u8, Vec<Vec<u8>>) {
        let seeds = Self::seeds(ncn, epoch);
        let (address, bump) = Pubkey::find_program_address(
            &seeds.iter().map(|s| s.as_slice()).collect::<Vec<_>>(),
            program_id,
        );
        (address, bump, seeds)
    }

    /// Validates that the account is owned by system program and matches expected PDA
    /// NCN reward receiver accounts are owned by system program, not the NCN program
    pub fn load(
        program_id: &Pubkey,
        account: &AccountInfo,
        ncn: &Pubkey,
        epoch: u64,
        expect_writable: bool,
    ) -> Result<(), ProgramError> {
        let system_program_id = system_program::id();
        let expected_pda = Self::find_program_address(program_id, ncn, epoch).0;
        check_load(
            &system_program_id,
            account,
            &expected_pda,
            None,
            expect_writable,
        )
    }

    /// Loads the account for closing (must be writable)
    pub fn load_to_close(
        program_id: &Pubkey,
        account_to_close: &AccountInfo,
        ncn: &Pubkey,
        epoch: u64,
    ) -> Result<(), ProgramError> {
        Self::load(program_id, account_to_close, ncn, epoch, true)
    }

    /// Closes the NCN reward receiver account and transfers remaining lamports
    ///
    /// This function transfers all lamports above minimum rent to the DAO wallet,
    /// then transfers the minimum rent to the account payer (for rent recovery).
    #[inline(always)]
    pub fn close<'a, 'info>(
        program_id: &Pubkey,
        ncn: &Pubkey,
        epoch: u64,
        ncn_reward_receiver: &'a AccountInfo<'info>,
        ncn_fee_wallet: &'a AccountInfo<'info>,
        account_payer: &'a AccountInfo<'info>,
    ) -> ProgramResult {
        let min_rent = Rent::get()?.minimum_balance(0);

        // Transfer excess lamports to DAO wallet
        let delta_lamports = ncn_reward_receiver.lamports().saturating_sub(min_rent);
        if delta_lamports > 0 {
            Self::transfer(
                program_id,
                ncn,
                epoch,
                ncn_reward_receiver,
                ncn_fee_wallet,
                delta_lamports,
            )?;
        }

        // Transfer minimum rent back to payer
        Self::transfer(
            program_id,
            ncn,
            epoch,
            ncn_reward_receiver,
            account_payer,
            min_rent,
        )
    }

    /// Transfers lamports from the NCN reward receiver using PDA authority
    #[inline(always)]
    pub fn transfer<'a, 'info>(
        program_id: &Pubkey,
        ncn: &Pubkey,
        epoch: u64,
        ncn_reward_receiver: &'a AccountInfo<'info>,
        to: &'a AccountInfo<'info>,
        lamports: u64,
    ) -> ProgramResult {
        let (ncn_reward_receiver_address, ncn_reward_receiver_bump, mut ncn_reward_receiver_seeds) =
            Self::find_program_address(program_id, ncn, epoch);
        ncn_reward_receiver_seeds.push(vec![ncn_reward_receiver_bump]);

        if ncn_reward_receiver_address.ne(ncn_reward_receiver.key) {
            msg!("Incorrect NCN reward receiver PDA");
            return Err(ProgramError::InvalidAccountData);
        }

        invoke_signed(
            &system_instruction::transfer(&ncn_reward_receiver_address, to.key, lamports),
            &[ncn_reward_receiver.clone(), to.clone()],
            &[ncn_reward_receiver_seeds
                .iter()
                .map(|seed| seed.as_slice())
                .collect::<Vec<&[u8]>>()
                .as_slice()],
        )?;
        Ok(())
    }
}

/// Wrapper struct for reward amounts in NCN reward router
/// This struct exists to encapsulate reward amount handling
#[derive(Default, Debug, Clone, PartialEq, Eq, Copy, Zeroable, ShankType, Pod)]
#[repr(C)]
pub struct NCNRewardRouterRewards {
    /// Reward amount in lamports
    rewards: PodU64,
}

impl NCNRewardRouterRewards {
    /// Gets the reward amount
    pub fn rewards(self) -> u64 {
        self.rewards.into()
    }
}

#[cfg(test)]
mod tests {
    use solana_program::pubkey::Pubkey;

    use super::*;
    use crate::{
        ballot_box::{Ballot, WeatherStatus},
        stake_weight::StakeWeights,
        utils::assert_ncn_program_error,
    };

    const TEST_EPOCH: u64 = 1;
    const TEST_CURRENT_SLOT: u64 = 100;
    const TEST_VALID_SLOTS_AFTER_CONSENSUS: u64 = 1000;

    pub fn get_test_ballot_box() -> BallotBox {
        let ncn = Pubkey::new_unique();
        let epoch = TEST_EPOCH;
        let current_slot = TEST_CURRENT_SLOT;
        let bump = 1;

        BallotBox::new(&ncn, epoch, bump, current_slot)
    }

    pub fn cast_test_vote(ballot_box: &mut BallotBox, stake_weight: u128, weather_status: u8) {
        let operator = Pubkey::new_unique();
        let ballot = Ballot::new(weather_status);
        let stake_weights = StakeWeights::snapshot(stake_weight).unwrap();

        ballot_box
            .cast_vote(
                &operator,
                &ballot,
                &stake_weights,
                TEST_CURRENT_SLOT,
                TEST_VALID_SLOTS_AFTER_CONSENSUS,
            )
            .unwrap();
    }

    pub fn get_test_operators(ballot_box: &BallotBox) -> Vec<Pubkey> {
        ballot_box
            .operator_votes()
            .iter()
            .filter(|vote| !vote.is_empty())
            .map(|votes| *votes.operator())
            .collect()
    }

    pub fn get_test_total_stake_weights(ballot_box: &BallotBox) -> StakeWeights {
        let mut total_stake_weights = StakeWeights::default();
        for vote in ballot_box.operator_votes() {
            total_stake_weights.increment(vote.stake_weights()).unwrap();
        }

        total_stake_weights
    }

    #[test]
    fn test_len() {
        use std::mem::size_of;

        let expected_total = size_of::<Pubkey>() // ncn
            + size_of::<PodU64>() // epoch
            + 1 // bump
            + size_of::<PodU64>() // slot_created
            + size_of::<PodU64>() // total_rewards
            + size_of::<PodU64>() // reward_pool
            + size_of::<PodU64>() // rewards_processed
            + 128 // reserved
            + size_of::<PodU16>() // last_vote_index
            + size_of::<PodU64>() // last_rewards_to_process
            + size_of::<PodU64>() // jito_dao_rewards
            + size_of::<PodU64>() // ncn_rewards
            + size_of::<PodU64>() // operator_vault_rewards
            + size_of::<OperatorVaultRewardRoute>() * 256; // operator_vault_reward_routes

        assert_eq!(size_of::<NCNRewardRouter>(), expected_total);
    }

    #[test]
    fn test_operator() {
        // Test case 1: Default operator (zero pubkey)
        let default_route = OperatorVaultRewardRoute::default();
        assert_eq!(*default_route.operator(), Pubkey::default());

        // Test case 2: Custom operator
        let custom_pubkey = Pubkey::new_unique();
        let custom_route = OperatorVaultRewardRoute::new(&custom_pubkey, 100).unwrap();
        assert_eq!(*custom_route.operator(), custom_pubkey);
    }

    #[test]
    fn test_increment_rewards_processed_zero() {
        // Create a new router
        let mut router = NCNRewardRouter::new(
            &Pubkey::new_unique(),
            1,   // epoch
            1,   // bump
            100, // slot_created
        );

        // Get initial rewards processed value
        let initial_rewards = router.rewards_processed();

        // Try to increment by 0
        let result = router.increment_rewards_processed(0);

        // Verify operation succeeded
        assert!(result.is_ok());

        // Verify rewards_processed hasn't changed
        assert_eq!(router.rewards_processed(), initial_rewards);
    }

    #[test]
    fn test_route_to_reward_pool_zero() {
        // Create a new router
        let mut router = NCNRewardRouter::new(
            &Pubkey::new_unique(),
            1,   // epoch
            1,   // bump
            100, // slot_created
        );

        // Record initial values
        let initial_total_rewards = router.total_rewards();
        let initial_reward_pool = router.reward_pool();

        // Try to route 0 rewards
        let result = router.route_to_reward_pool(0);

        // Verify operation succeeded
        assert!(result.is_ok());

        // Verify state hasn't changed
        assert_eq!(router.total_rewards(), initial_total_rewards);
        assert_eq!(router.reward_pool(), initial_reward_pool);
    }

    #[test]
    fn test_has_rewards() {
        // Test case 1: No rewards in any group
        let empty_route = OperatorVaultRewardRoute::default();
        assert!(!empty_route.has_rewards().unwrap());

        // Test case 2: has some rewards
        let route = OperatorVaultRewardRoute::new(&Pubkey::new_unique(), 100).unwrap();
        assert!(route.has_rewards().unwrap());

        // Test case 4: Zero rewards in all groups
        let zero_rewards_route = OperatorVaultRewardRoute::new(&Pubkey::new_unique(), 0).unwrap();
        assert!(!zero_rewards_route.has_rewards().unwrap());
    }

    #[test]
    fn test_route_incoming_rewards() {
        let mut router = NCNRewardRouter::new(&Pubkey::new_unique(), 1, 1, 100);

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
    fn test_route_reward_pool() {
        const INCOMING_REWARDS: u64 = 1000;

        let mut router = NCNRewardRouter::new(
            &Pubkey::new_unique(), // ncn
            1,                     // ncn_epoch
            1,                     // bump
            100,                   // slot_created
        );

        // Fees
        let fees = Fees::new(100, 1).unwrap();

        // Route incoming rewards
        router.route_incoming_rewards(0, INCOMING_REWARDS).unwrap();

        assert_eq!(router.total_rewards(), INCOMING_REWARDS);
        assert_eq!(router.reward_pool(), INCOMING_REWARDS);

        router.route_reward_pool(&fees).unwrap();

        assert_eq!(router.total_rewards(), INCOMING_REWARDS);
        assert_eq!(router.reward_pool(), 0);
        assert_eq!(router.jito_dao_rewards(), 40); // 4% of 1000
        assert_eq!(router.ncn_rewards(), 10);
    }

    #[test]
    fn test_route_reward_pool_remainder() {
        const INCOMING_REWARDS: u64 = 1000;

        let mut router = NCNRewardRouter::new(
            &Pubkey::new_unique(), // ncn
            1,                     // ncn_epoch
            1,                     // bump
            100,                   // slot_created
        );

        // Fees
        let fees = Fees::new(100, 1).unwrap();

        // Route incoming rewards
        router.route_incoming_rewards(0, INCOMING_REWARDS).unwrap();

        assert_eq!(router.total_rewards(), INCOMING_REWARDS);
        assert_eq!(router.reward_pool(), INCOMING_REWARDS);

        router.route_reward_pool(&fees).unwrap();

        assert_eq!(router.total_rewards(), INCOMING_REWARDS);
        assert_eq!(router.reward_pool(), 0);

        assert_eq!(router.jito_dao_rewards(), 40); // 4% of 1000
        assert_eq!(router.ncn_rewards(), 10);
        assert_eq!(router.operator_vault_rewards(), 950);
    }

    #[test]
    fn test_rounding() {
        const INCOMING_REWARDS: u64 = 1000;

        let mut router = NCNRewardRouter::new(
            &Pubkey::new_unique(), // ncn
            1,                     // ncn_epoch
            1,                     // bump
            100,                   // slot_created
        );

        // Fees - all base groups and ncn groups
        let fees = Fees::new(19, 1).unwrap();

        // Route incoming rewards
        router.route_incoming_rewards(0, INCOMING_REWARDS).unwrap();

        assert_eq!(router.total_rewards(), INCOMING_REWARDS);
        assert_eq!(router.reward_pool(), INCOMING_REWARDS);

        router.route_reward_pool(&fees).unwrap();

        assert_eq!(router.total_rewards(), INCOMING_REWARDS);
        assert_eq!(router.reward_pool(), 0);

        assert_eq!(router.jito_dao_rewards(), 40); // 4% of 1000
        assert_eq!(router.ncn_rewards(), 1);
        assert_eq!(router.operator_vault_rewards(), 959);
    }

    #[test]
    fn test_route_to_operators_consensus_not_reached() {
        let mut router = NCNRewardRouter::new(
            &Pubkey::new_unique(), // ncn
            1,                     // ncn_epoch
            1,                     // bump
            100,                   // slot_created
        );
        router.operator_vault_rewards = PodU64::from(100);

        let ballot_box = get_test_ballot_box();
        let result = router.route_operator_vault_rewards(&ballot_box, 100);

        assert_ncn_program_error(result, NCNProgramError::ConsensusNotReached);
    }

    #[test]
    fn test_route_to_operators() {
        // TODO: Start from here
        const INCOMING_REWARDS: u64 = 1000;
        const NUM_OPERATORS: u64 = 10;
        let mut router = NCNRewardRouter::new(
            &Pubkey::new_unique(), // ncn
            1,                     // ncn_epoch
            1,                     // bump
            100,                   // slot_created
        );
        router.operator_vault_rewards = PodU64::from(INCOMING_REWARDS);

        let (ballot_box, operators) = {
            let mut ballot_box = get_test_ballot_box();
            for _ in 0..NUM_OPERATORS {
                cast_test_vote(&mut ballot_box, 200, WeatherStatus::Sunny as u8);
            }
            let operators = get_test_operators(&ballot_box);
            let total_stake_weights = get_test_total_stake_weights(&ballot_box);
            ballot_box
                .tally_votes(total_stake_weights.stake_weight(), TEST_CURRENT_SLOT)
                .unwrap();
            (ballot_box, operators)
        };

        router
            .route_operator_vault_rewards(&ballot_box, 100)
            .unwrap();

        for operator in operators.iter() {
            let route = router.oprtator_vault_reward_route(operator).unwrap();
            assert_eq!(route.rewards().unwrap(), INCOMING_REWARDS / NUM_OPERATORS);
        }
        // remainder goes to ncn
        assert_eq!(router.ncn_rewards(), 0);
    }

    #[test]
    fn test_route_to_operators_with_wrong_vote() {
        const INCOMING_REWARDS: u64 = 1000;
        const NUM_CORRECT_OPERATORS: u64 = 7;
        const NUM_WRONG_OPERATORS: u64 = 3;

        let mut router = NCNRewardRouter::new(
            &Pubkey::new_unique(), // ncn
            1,                     // ncn_epoch
            1,                     // bump
            100,                   // slot_created
        );
        router.operator_vault_rewards = PodU64::from(INCOMING_REWARDS);

        let (ballot_box, operators) = {
            let mut ballot_box = get_test_ballot_box();
            for _ in 0..NUM_CORRECT_OPERATORS {
                cast_test_vote(&mut ballot_box, 200, WeatherStatus::Sunny as u8);
            }
            for _ in 0..NUM_WRONG_OPERATORS {
                cast_test_vote(&mut ballot_box, 200, WeatherStatus::Cloudy as u8);
            }
            let operators = get_test_operators(&ballot_box);
            let total_stake_weights = get_test_total_stake_weights(&ballot_box);
            ballot_box
                .tally_votes(total_stake_weights.stake_weight(), TEST_CURRENT_SLOT)
                .unwrap();
            (ballot_box, operators)
        };

        router
            .route_operator_vault_rewards(&ballot_box, 100)
            .unwrap();

        let winning_ballot = ballot_box.get_winning_ballot_tally().unwrap();
        let winning_stake_weight = winning_ballot.stake_weights().stake_weight();
        let expected_reward_per_operator = INCOMING_REWARDS as u128 * 200 / winning_stake_weight;

        let mut correct_vote_operators = 0;
        for operator in operators.iter() {
            if let Ok(route) = router.oprtator_vault_reward_route(operator) {
                assert_eq!(
                    route.rewards().unwrap(),
                    expected_reward_per_operator as u64
                );
                correct_vote_operators += 1;
            }
        }
        assert_eq!(correct_vote_operators, NUM_CORRECT_OPERATORS);
        let remainder = router.operator_vault_rewards() + router.ncn_rewards();
        assert_eq!(
            remainder,
            INCOMING_REWARDS - (expected_reward_per_operator as u64 * NUM_CORRECT_OPERATORS)
        );
    }

    #[test]
    fn test_route_to_max_operators() {
        const INCOMING_REWARDS: u64 = 256_000;

        let mut router = NCNRewardRouter::new(
            &Pubkey::new_unique(), // ncn
            1,                     // ncn_epoch
            1,                     // bump
            100,                   // slot_created
        );

        router.operator_vault_rewards = PodU64::from(INCOMING_REWARDS);

        let (ballot_box, operators) = {
            let mut ballot_box = get_test_ballot_box();

            for _ in 0..256 {
                cast_test_vote(&mut ballot_box, 200, WeatherStatus::Sunny as u8);
            }

            let total_stake_weights = get_test_total_stake_weights(&ballot_box);

            ballot_box
                .tally_votes(total_stake_weights.stake_weight(), TEST_CURRENT_SLOT)
                .unwrap();

            (ballot_box, get_test_operators(&ballot_box))
        };

        router
            .route_operator_vault_rewards(&ballot_box, 1000)
            .unwrap();

        assert!(!router.still_routing());

        for operator in operators.iter() {
            let route = router.oprtator_vault_reward_route(operator).unwrap();
            assert_eq!(route.rewards().unwrap(), 1000);
        }
        assert_eq!(router.ncn_rewards(), 0);
    }

    #[test]
    fn test_route_with_interruption() {
        const INCOMING_REWARDS: u64 = 256_000;

        let mut router = NCNRewardRouter::new(
            &Pubkey::new_unique(), // ncn
            1,                     // ncn_epoch
            1,                     // bump
            100,                   // slot_created
        );
        router.operator_vault_rewards = PodU64::from(INCOMING_REWARDS);

        let (ballot_box, operators) = {
            let mut ballot_box = get_test_ballot_box();

            for _ in 0..256 {
                cast_test_vote(&mut ballot_box, 200, WeatherStatus::Sunny as u8);
            }

            let total_stake_weights = get_test_total_stake_weights(&ballot_box);

            ballot_box
                .tally_votes(total_stake_weights.stake_weight(), TEST_CURRENT_SLOT)
                .unwrap();

            (ballot_box, get_test_operators(&ballot_box))
        };

        assert_eq!(operators.len(), 256);

        router.route_operator_vault_rewards(&ballot_box, 5).unwrap();

        assert!(router.still_routing());

        router
            .route_operator_vault_rewards(&ballot_box, 256 * 8)
            .unwrap();

        assert!(!router.still_routing());

        for operator in operators.iter() {
            let route = router.oprtator_vault_reward_route(operator).unwrap();
            assert_eq!(route.rewards().unwrap(), 1000);
        }
    }

    #[test]
    fn test_route_with_0_iterations() {
        const INCOMING_REWARDS: u64 = 256_000;

        let mut router = NCNRewardRouter::new(
            &Pubkey::new_unique(), // ncn
            1,                     // ncn_epoch
            1,                     // bump
            100,                   // slot_created
        );
        router.operator_vault_rewards = PodU64::from(INCOMING_REWARDS);

        let (ballot_box, operators) = {
            let mut ballot_box = get_test_ballot_box();

            for _ in 0..256 {
                cast_test_vote(&mut ballot_box, 200, WeatherStatus::Sunny as u8);
            }

            let total_stake_weights = get_test_total_stake_weights(&ballot_box);

            ballot_box
                .tally_votes(total_stake_weights.stake_weight(), TEST_CURRENT_SLOT)
                .unwrap();

            (ballot_box, get_test_operators(&ballot_box))
        };

        assert_eq!(operators.len(), 256);

        router.route_operator_vault_rewards(&ballot_box, 0).unwrap();

        assert!(router.still_routing());

        for _ in 0..256 {
            router.route_operator_vault_rewards(&ballot_box, 0).unwrap();
        }

        assert!(!router.still_routing());

        for operator in operators.iter() {
            let route = router.oprtator_vault_reward_route(operator).unwrap();
            assert_eq!(route.rewards().unwrap(), 1000);
        }
    }

    #[test]
    fn test_distribute_operator_vault_reward_route_not_found() {
        // Create a new router
        let mut router = NCNRewardRouter::new(
            &Pubkey::new_unique(),
            1,   // epoch
            1,   // bump
            100, // slot_created
        );

        // Try to distribute rewards for a non-existent operator
        let non_existent_operator = Pubkey::new_unique();
        let result = router.distribute_operator_vault_reward_route(&non_existent_operator);

        // Verify we get the expected error
        assert_eq!(result.unwrap_err(), NCNProgramError::OperatorRewardNotFound);
    }
}
