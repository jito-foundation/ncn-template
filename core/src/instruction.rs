use borsh::{BorshDeserialize, BorshSerialize};
use shank::ShankInstruction;
use solana_program::pubkey::Pubkey;

use crate::config::ConfigAdminRole;

/// Represents all instructions supported by the NCN Program
/// Each instruction specifies the accounts it requires and any parameters
/// The instruction variants are organized into logical sections:
/// - Global: Program initialization and configuration
/// - Snapshot: Creating snapshots of validator and operator state
/// - Vote: Consensus voting mechanism
/// - Route and Distribute: Rewards distribution
/// - Admin: Administrative operations
#[rustfmt::skip]
#[derive(Debug, BorshSerialize, BorshDeserialize, ShankInstruction)]
pub enum NCNProgramInstruction {

    // ---------------------------------------------------- //
    //                         GLOBAL                       //
    // ---------------------------------------------------- //
    /// Initialize the config account for the NCN program
    /// Sets up the basic program parameters
    #[account(0, writable, name = "config")]
    #[account(1, name = "ncn")]
    #[account(2, name = "ncn_fee_wallet")]
    #[account(3, signer, name = "ncn_admin")]
    #[account(4, name = "tie_breaker_admin")]
    #[account(5, writable, name = "account_payer")]
    #[account(6, name = "system_program")]
    InitializeConfig {
        /// Number of epochs before voting is considered stalled
        epochs_before_stall: u64,
        /// Number of epochs after consensus before accounts can be closed
        epochs_after_consensus_before_close: u64,
        /// Number of slots after consensus where voting is still valid
        valid_slots_after_consensus: u64,
        /// NCN fee basis points (bps) for the NCN program
        ncn_fee_bps: u16,
    },

    /// Initializes the vault registry account to track validator vaults
    #[account(0, name = "config")]
    #[account(1, writable, name = "vault_registry")]
    #[account(2, name = "ncn")]
    #[account(3, writable, name = "account_payer")]
    #[account(4, name = "system_program")]
    InitializeVaultRegistry,

    /// Resizes the vault registry account
    #[account(0, name = "config")]
    #[account(1, writable, name = "vault_registry")]
    #[account(2, name = "ncn")]
    #[account(3, writable, name = "account_payer")]
    #[account(4, name = "system_program")]
    ReallocVaultRegistry,

    /// Registers a vault to the vault registry
    #[account(0, name = "config")]
    #[account(1, writable, name = "vault_registry")]
    #[account(2, name = "ncn")]
    #[account(3, name = "vault")]
    #[account(4, name = "ncn_vault_ticket")]
    RegisterVault,

    // ---------------------------------------------------- //
    //                       SNAPSHOT                       //
    // ---------------------------------------------------- //
    /// Initializes the Epoch State account for a specific epoch
    /// The epoch state tracks the status of an epoch
    #[account(0, name = "epoch_marker")]
    #[account(1, writable, name = "epoch_state")]
    #[account(2, name = "config")]
    #[account(3, name = "ncn")]
    #[account(4, writable, name = "account_payer")]
    #[account(5, name = "system_program")]
    InitializeEpochState {
        /// Target epoch for initialization
        epoch: u64,
    },


    /// Initializes the weight table for a given epoch
    #[account(0, name = "epoch_marker")]
    #[account(1, name = "epoch_state")]
    #[account(2, name = "vault_registry")]
    #[account(3, name = "ncn")]
    #[account(4, writable, name = "weight_table")]
    #[account(5, writable, name = "account_payer")]
    #[account(6, name = "system_program")]
    InitializeWeightTable{
        /// Target epoch for the weight table
        epoch: u64,
    },


    /// Set weights for the weight table using the vault registry
    #[account(0, writable, name = "epoch_state")]
    #[account(1, name = "ncn")]
    #[account(2, name = "vault_registry")]
    #[account(3, writable, name = "weight_table")]
    SetEpochWeights{
        epoch: u64,
    },

    /// Resizes the weight table account
    #[account(0, writable, name = "epoch_state")]
    #[account(1, name = "config")]
    #[account(2, writable, name = "weight_table")]
    #[account(3, name = "ncn")]
    #[account(4, name = "vault_registry")]
    #[account(5, writable, name = "account_payer")]
    #[account(6, name = "system_program")]
    ReallocWeightTable {
        epoch: u64,
    },



    /// Initializes the Epoch Snapshot
    #[account(0, name = "epoch_marker")]
    #[account(1, writable, name = "epoch_state")]
    #[account(2, name = "config")]
    #[account(3, name = "ncn")]
    #[account(4, name = "weight_table")]
    #[account(5, writable, name = "epoch_snapshot")]
    #[account(6, writable, name = "account_payer")]
    #[account(7, name = "system_program")]
    InitializeEpochSnapshot{
        epoch: u64,
    },

    /// Initializes the Operator Snapshot
    #[account(0, name = "epoch_marker")]
    #[account(1, writable, name = "epoch_state")]
    #[account(2, name = "config")]
    #[account(3, name = "restaking_config")]
    #[account(4, name = "ncn")]
    #[account(5, name = "operator")]
    #[account(6, name = "ncn_operator_state")]
    #[account(7, writable, name = "epoch_snapshot")]
    #[account(8, writable, name = "operator_snapshot")]
    #[account(9, writable, name = "account_payer")]
    #[account(10, name = "system_program")]
    InitializeOperatorSnapshot{
        epoch: u64,
    },
    
    /// Snapshots the vault operator delegation
    #[account(0, writable, name = "epoch_state")]
    #[account(1, name = "config")]
    #[account(2, name = "restaking_config")]
    #[account(3, name = "ncn")]
    #[account(4, name = "operator")]
    #[account(5, name = "vault")]
    #[account(6, name = "vault_ncn_ticket")]
    #[account(7, name = "ncn_vault_ticket")]
    #[account(8, name = "vault_operator_delegation")]
    #[account(9, name = "weight_table")]
    #[account(10, writable, name = "epoch_snapshot")]
    #[account(11, writable, name = "operator_snapshot")]
    SnapshotVaultOperatorDelegation{
        epoch: u64,
    },

    // ---------------------------------------------------- //
    //                         VOTE                         //
    // ---------------------------------------------------- //
    /// Initializes the ballot box for an NCN
    #[account(0, name = "epoch_marker")]
    #[account(1, name = "epoch_state")]
    #[account(2, name = "config")]
    #[account(3, writable, name = "ballot_box")]
    #[account(4, name = "ncn")]
    #[account(5, writable, name = "account_payer")]
    #[account(6, name = "system_program")]
    #[account(7, writable, name = "consensus_result")]
    InitializeBallotBox {
        epoch: u64,
    },

    /// Resizes the ballot box account
    #[account(0, writable, name = "epoch_state")]
    #[account(1, name = "config")]
    #[account(2, writable, name = "ballot_box")]
    #[account(3, name = "ncn")]
    #[account(4, writable, name = "account_payer")]
    #[account(5, name = "system_program")]
    ReallocBallotBox {
        epoch: u64,
    },

    /// Cast a vote for a merkle root
    #[account(0, writable, name = "epoch_state")]
    #[account(1, name = "config")]
    #[account(2, writable, name = "ballot_box")]
    #[account(3, name = "ncn")]
    #[account(4, name = "epoch_snapshot")]
    #[account(5, name = "operator_snapshot")]
    #[account(6, name = "operator")]
    #[account(7, signer, name = "operator_voter")]
    #[account(8, writable, name = "consensus_result")]
    CastVote {
        weather_status: u8,
        epoch: u64,
    },

    // ---------------------------------------------------- //
    //                ROUTE AND DISTRIBUTE                  //
    // ---------------------------------------------------- //
    /// Initializes the NCN reward router

    #[account(0, name = "epoch_marker")]
    #[account(1, name = "epoch_state")]
    #[account(2, name = "ncn")]
    #[account(3, writable, name = "ncn_reward_router")]
    #[account(4, writable, name = "ncn_reward_receiver")]
    #[account(5, writable, name = "account_payer")]
    #[account(6, name = "system_program")]
    InitializeNCNRewardRouter{
        epoch: u64,
    },

    /// Resizes the NCN reward router account
    #[account(0, writable, name = "epoch_state")]
    #[account(1, name = "config")]
    #[account(2, writable, name = "ncn_reward_router")]
    #[account(3, name = "ncn")]
    #[account(4, writable, name = "account_payer")]
    #[account(5, name = "system_program")]
    ReallocNCNRewardRouter {
        epoch: u64,
    },

    /// Routes NCN reward router
    #[account(0, writable, name = "epoch_state")]
    #[account(1, name = "config")]
    #[account(2, name = "ncn")]
    #[account(3, name = "epoch_snapshot")]
    #[account(4, name = "ballot_box")]
    #[account(5, writable, name = "ncn_reward_router")]
    #[account(6, writable, name = "ncn_reward_receiver")]
    RouteNCNRewards{
        max_iterations: u16,
        epoch: u64,
    },

    /// Distributes Protocol rewards
    #[account(0, writable, name = "epoch_state")]
    #[account(1, name = "config")]
    #[account(2, name = "ncn")]
    #[account(3, writable, name = "ncn_reward_router")]
    #[account(4, writable, name = "ncn_reward_receiver")]
    #[account(5, writable, name = "protocol_fee_wallet")]
    #[account(6, name = "system_program")]
    DistributeProtocolRewards{
        epoch: u64,
    },

    /// Distributes NCN rewards
    #[account(0, writable, name = "epoch_state")]
    #[account(1, name = "config")]
    #[account(2, name = "ncn")]
    #[account(3, writable, name = "ncn_reward_router")]
    #[account(4, writable, name = "ncn_reward_receiver")]
    #[account(5, writable, name = "ncn_fee_wallet")]
    #[account(6, name = "system_program")]
    DistributeNCNRewards{
        epoch: u64,
    },

    #[account(0, name = "epoch_marker")]
    #[account(1, writable, name = "epoch_state")]
    #[account(2, name = "ncn")]
    #[account(3, name = "operator")]
    #[account(4, name = "operator_snapshot")]
    #[account(5, writable, name = "operator_vault_reward_router")]
    #[account(6, writable, name = "operator_vault_reward_receiver")]
    #[account(7, writable, name = "account_payer")]
    #[account(8, name = "system_program")]
    InitializeOperatorVaultRewardRouter{
        epoch: u64,
    },

    /// Distributes base ncn reward routes
    #[account(0, writable, name = "epoch_state")]
    #[account(1, name = "config")]
    #[account(2, name = "ncn")]
    #[account(3, name = "operator")]
    #[account(4, writable, name = "ncn_reward_router")]
    #[account(5, writable, name = "ncn_reward_receiver")]
    #[account(6, name = "operator_vault_reward_router")]
    #[account(7, writable, name = "operator_vault_reward_receiver")]
    #[account(8, name = "system_program")]
    DistributeOperatorVaultRewardRoute{
        epoch: u64,
    },

    /// Routes ncn reward router
    #[account(0, writable, name = "epoch_state")]
    #[account(1, name = "ncn")]
    #[account(2, name = "operator")]
    #[account(3, name = "operator_snapshot")]
    #[account(4, writable, name = "operator_vault_reward_router")]
    #[account(5, writable, name = "operator_vault_reward_receiver")]
    RouteOperatorVaultRewards{
        max_iterations: u16,
        epoch: u64,
    },



    /// Close an epoch account
    #[account(0, writable, name = "epoch_marker")]
    #[account(1, writable, name = "epoch_state")]
    #[account(2, name = "config")]
    #[account(3, name = "ncn")]
    #[account(4, writable, name = "account_to_close")]
    #[account(5, writable, name = "account_payer")]
    #[account(6, name = "system_program")]
    #[account(7, writable, optional, name = "ncn_fee_wallet")]
    #[account(8, writable, optional, name = "receiver_to_close")]
    CloseEpochAccount {
        epoch: u64,
    },

    /// Distributes ncn operator rewards
    #[account(0, writable, name = "epoch_state")]
    #[account(1, name = "config")]
    #[account(2, name = "ncn")]
    #[account(3, writable, name = "operator")]
    #[account(4, writable, name = "operator_snapshot")]
    #[account(5, writable, name = "operator_vault_reward_router")]
    #[account(6, writable, name = "operator_vault_reward_receiver")]
    #[account(7, name = "system_program")]
    DistributeOperatorRewards{
        epoch: u64,
    },

    /// Distributes vault rewards
    #[account(0, writable, name = "epoch_state")]
    #[account(1, name = "config")]
    #[account(2, name = "ncn")]
    #[account(3, name = "operator")]
    #[account(4, writable, name = "vault")]
    #[account(5, writable, name = "operator_snapshot")]
    #[account(6, writable, name = "operator_vault_reward_router")]
    #[account(7, writable, name = "operator_vault_reward_receiver")]
    #[account(8, name = "system_program")]
    DistributeVaultRewards{
        epoch: u64,
    },


    // ---------------------------------------------------- //
    //                        ADMIN                         //
    // ---------------------------------------------------- //
    /// Updates NCN Config parameters
    #[account(0, writable, name = "config")]
    #[account(1, name = "ncn")]
    #[account(2, signer, name = "ncn_admin")]
    AdminSetParameters {
        starting_valid_epoch: Option<u64>,
        epochs_before_stall: Option<u64>,
        epochs_after_consensus_before_close: Option<u64>,
        valid_slots_after_consensus: Option<u64>,
    },


    /// Sets a new secondary admin for the NCN
    #[account(0, writable, name = "config")]
    #[account(1, name = "ncn")]
    #[account(2, signer, name = "ncn_admin")]
    #[account(3, name = "new_admin")]
    AdminSetNewAdmin {
        role: ConfigAdminRole,
    },

    /// Set tie breaker in case of stalled voting
    #[account(0, writable, name = "epoch_state")]
    #[account(1, name = "config")]
    #[account(2, writable, name = "ballot_box")]
    #[account(3, name = "ncn")]
    #[account(4, signer, name = "tie_breaker_admin")]
    AdminSetTieBreaker {
        weather_status: u8,
        epoch: u64,
    },

    /// Sets a weight
    #[account(0, writable, name = "epoch_state")]
    #[account(1, name = "ncn")]
    #[account(2, writable, name = "weight_table")]
    #[account(3, signer, name = "weight_table_admin")]
    AdminSetWeight{
        st_mint: Pubkey,
        weight: u128,
        epoch: u64,
    },

    /// Registers a new ST mint in the Vault Registry
    #[account(0, name = "config")]
    #[account(1, name = "ncn")]
    #[account(2, name = "st_mint")]
    #[account(3, writable, name = "vault_registry")]
    #[account(4, signer, writable, name = "admin")]
    AdminRegisterStMint{
        weight: Option<u128>,
    },

    /// Updates an ST mint in the Vault Registry
    #[account(0, name = "config")]
    #[account(1, name = "ncn")]
    #[account(2, writable, name = "vault_registry")]
    #[account(3, signer, writable, name = "admin")]
    AdminSetStMint{
        st_mint: Pubkey,
        weight: Option<u128>,
    },
}
