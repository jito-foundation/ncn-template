use jito_bytemuck::AccountDeserialize;
use jito_restaking_core::{
    config::Config, ncn_operator_state::NcnOperatorState, ncn_vault_ticket::NcnVaultTicket,
};
use jito_vault_core::{
    vault_ncn_ticket::VaultNcnTicket, vault_operator_delegation::VaultOperatorDelegation,
};
use ncn_program_client::{
    instructions::{
        AdminRegisterStMintBuilder, AdminSetNewAdminBuilder, AdminSetParametersBuilder,
        AdminSetStMintBuilder, AdminSetTieBreakerBuilder, AdminSetWeightBuilder, CastVoteBuilder,
        CloseEpochAccountBuilder, DistributeNCNRewardsBuilder, DistributeOperatorRewardsBuilder,
        DistributeOperatorVaultRewardRouteBuilder, DistributeProtocolRewardsBuilder,
        DistributeVaultRewardsBuilder, InitializeBallotBoxBuilder, InitializeConfigBuilder,
        InitializeEpochSnapshotBuilder, InitializeEpochStateBuilder,
        InitializeNCNRewardRouterBuilder, InitializeOperatorSnapshotBuilder,
        InitializeOperatorVaultRewardRouterBuilder, InitializeVaultRegistryBuilder,
        InitializeWeightTableBuilder, ReallocBallotBoxBuilder, ReallocNCNRewardRouterBuilder,
        ReallocVaultRegistryBuilder, ReallocWeightTableBuilder, RegisterVaultBuilder,
        RouteNCNRewardsBuilder, RouteOperatorVaultRewardsBuilder, SetEpochWeightsBuilder,
        SnapshotVaultOperatorDelegationBuilder,
    },
    types::ConfigAdminRole,
};
use ncn_program_core::{
    account_payer::AccountPayer,
    ballot_box::BallotBox,
    config::Config as NcnConfig,
    consensus_result::ConsensusResult,
    constants::MAX_REALLOC_BYTES,
    epoch_marker::EpochMarker,
    epoch_snapshot::{EpochSnapshot, OperatorSnapshot},
    epoch_state::EpochState,
    error::NCNProgramError,
    fees::FeeConfig,
    ncn_reward_router::{NCNRewardReceiver, NCNRewardRouter},
    operator_vault_reward_router::{OperatorVaultRewardReceiver, OperatorVaultRewardRouter},
    vault_registry::VaultRegistry,
    weight_table::WeightTable,
};
use solana_program::{
    instruction::InstructionError, native_token::sol_to_lamports, pubkey::Pubkey,
    system_instruction::transfer,
};
use solana_program_test::{BanksClient, ProgramTestBanksClientExt};
use solana_sdk::{
    commitment_config::CommitmentLevel,
    compute_budget::ComputeBudgetInstruction,
    hash::Hash,
    msg,
    signature::{Keypair, Signer},
    system_program,
    transaction::{Transaction, TransactionError},
};

use super::restaking_client::NcnRoot;
use crate::fixtures::{TestError, TestResult};

/// A client for interacting with the NCN program in integration tests.
/// Provides helper methods for initializing accounts, fetching state, and sending transactions.
pub struct NCNProgramClient {
    banks_client: BanksClient,
    payer: Keypair,
}

impl NCNProgramClient {
    /// Creates a new NCN program client.
    pub const fn new(banks_client: BanksClient, payer: Keypair) -> Self {
        Self {
            banks_client,
            payer,
        }
    }

    /// Processes a transaction using the BanksClient with processed commitment level.
    pub async fn process_transaction(&mut self, tx: &Transaction) -> TestResult<()> {
        self.banks_client
            .process_transaction_with_preflight_and_commitment(
                tx.clone(),
                CommitmentLevel::Processed,
            )
            .await?;
        Ok(())
    }

    /// Airdrops SOL to a specified public key.
    pub async fn airdrop(&mut self, to: &Pubkey, sol: f64) -> TestResult<()> {
        let blockhash = self.banks_client.get_latest_blockhash().await?;
        let new_blockhash = self
            .banks_client
            .get_new_latest_blockhash(&blockhash)
            .await
            .unwrap();
        self.banks_client
            .process_transaction_with_preflight_and_commitment(
                Transaction::new_signed_with_payer(
                    &[transfer(&self.payer.pubkey(), to, sol_to_lamports(sol))],
                    Some(&self.payer.pubkey()),
                    &[&self.payer],
                    new_blockhash,
                ),
                CommitmentLevel::Processed,
            )
            .await?;
        Ok(())
    }

    /// Sets up the NCN program by initializing the config and vault registry.
    pub async fn setup_ncn_program(&mut self, ncn_root: &NcnRoot) -> TestResult<()> {
        self.do_initialize_config(ncn_root.ncn_pubkey, &ncn_root.ncn_admin)
            .await?;

        self.do_full_initialize_vault_registry(ncn_root.ncn_pubkey)
            .await?;

        Ok(())
    }

    pub async fn get_best_latest_blockhash(&mut self) -> TestResult<Hash> {
        let blockhash = self.banks_client.get_latest_blockhash().await?;
        let new_blockhash = self
            .banks_client
            .get_new_latest_blockhash(&blockhash)
            .await?;

        Ok(new_blockhash)
    }

    /// Fetches the EpochMarker account for a given NCN and epoch.
    pub async fn get_epoch_marker(&mut self, ncn: Pubkey, epoch: u64) -> TestResult<EpochMarker> {
        let epoch_marker = EpochMarker::find_program_address(&ncn_program::id(), &ncn, epoch).0;
        let raw_account = self.banks_client.get_account(epoch_marker).await?.unwrap();
        Ok(*EpochMarker::try_from_slice_unchecked(raw_account.data.as_slice()).unwrap())
    }

    /// Fetches the NCN Config account for a given NCN pubkey.
    pub async fn get_ncn_config(&mut self, ncn_pubkey: Pubkey) -> TestResult<NcnConfig> {
        let config_pda = NcnConfig::find_program_address(&ncn_program::id(), &ncn_pubkey).0;
        let config = self.banks_client.get_account(config_pda).await?.unwrap();
        Ok(*NcnConfig::try_from_slice_unchecked(config.data.as_slice()).unwrap())
    }

    /// Fetches the VaultRegistry account for a given NCN pubkey.
    pub async fn get_vault_registry(&mut self, ncn_pubkey: Pubkey) -> TestResult<VaultRegistry> {
        let vault_registry_pda =
            VaultRegistry::find_program_address(&ncn_program::id(), &ncn_pubkey).0;
        let vault_registry = self
            .banks_client
            .get_account(vault_registry_pda)
            .await?
            .unwrap();
        Ok(*VaultRegistry::try_from_slice_unchecked(vault_registry.data.as_slice()).unwrap())
    }

    /// Fetches the EpochState account for a given NCN and epoch.
    pub async fn get_epoch_state(&mut self, ncn: Pubkey, epoch: u64) -> TestResult<EpochState> {
        let epoch_state = EpochState::find_program_address(&ncn_program::id(), &ncn, epoch).0;
        let raw_account = self.banks_client.get_account(epoch_state).await?.unwrap();
        Ok(*EpochState::try_from_slice_unchecked(raw_account.data.as_slice()).unwrap())
    }

    /// Fetches the WeightTable account for a given NCN and epoch.
    #[allow(dead_code)]
    pub async fn get_weight_table(
        &mut self,
        ncn: Pubkey,
        ncn_epoch: u64,
    ) -> TestResult<WeightTable> {
        let address = WeightTable::find_program_address(&ncn_program::id(), &ncn, ncn_epoch).0;

        let raw_account = self.banks_client.get_account(address).await?.unwrap();

        let account = WeightTable::try_from_slice_unchecked(raw_account.data.as_slice()).unwrap();

        Ok(*account)
    }

    /// Fetches the EpochSnapshot account for a given NCN and epoch.
    pub async fn get_epoch_snapshot(
        &mut self,
        ncn: Pubkey,
        ncn_epoch: u64,
    ) -> TestResult<EpochSnapshot> {
        let address = EpochSnapshot::find_program_address(&ncn_program::id(), &ncn, ncn_epoch).0;

        let raw_account = self.banks_client.get_account(address).await?.unwrap();

        let account = EpochSnapshot::try_from_slice_unchecked(raw_account.data.as_slice()).unwrap();

        Ok(*account)
    }

    /// Fetches the OperatorSnapshot account for a given operator, NCN, and epoch.
    #[allow(dead_code)]
    pub async fn get_operator_snapshot(
        &mut self,
        operator: Pubkey,
        ncn: Pubkey,
        ncn_epoch: u64,
    ) -> TestResult<OperatorSnapshot> {
        let address =
            OperatorSnapshot::find_program_address(&ncn_program::id(), &operator, &ncn, ncn_epoch)
                .0;

        let raw_account = self.banks_client.get_account(address).await?.unwrap();

        let account =
            OperatorSnapshot::try_from_slice_unchecked(raw_account.data.as_slice()).unwrap();

        Ok(*account)
    }

    /// Fetches the BallotBox account for a given NCN and epoch.
    pub async fn get_ballot_box(&mut self, ncn: Pubkey, epoch: u64) -> TestResult<BallotBox> {
        let address = BallotBox::find_program_address(&ncn_program::id(), &ncn, epoch).0;
        let raw_account = self.banks_client.get_account(address).await?.unwrap();
        Ok(*BallotBox::try_from_slice_unchecked(raw_account.data.as_slice()).unwrap())
    }

    /// Fetches the ConsensusResult account for a given NCN and epoch.
    pub async fn get_consensus_result(
        &mut self,
        ncn: Pubkey,
        epoch: u64,
    ) -> TestResult<ConsensusResult> {
        let address = ConsensusResult::find_program_address(&ncn_program::id(), &ncn, epoch).0;

        let raw_account = self.banks_client.get_account(address).await?.unwrap();

        Ok(*ConsensusResult::try_from_slice_unchecked(raw_account.data.as_slice()).unwrap())
    }

    /// Initializes the NCN config account and airdrops funds to the account payer.
    pub async fn do_initialize_config(
        &mut self,
        ncn: Pubkey,
        ncn_admin: &Keypair,
    ) -> TestResult<()> {
        // Setup Payer
        self.airdrop(&self.payer.pubkey(), 1.0).await?;

        // Setup account payer
        let (account_payer, _, _) = AccountPayer::find_program_address(&ncn_program::id(), &ncn);
        self.airdrop(&account_payer, 100.0).await?;

        let ncn_admin_pubkey = ncn_admin.pubkey();

        let ncn_fee_wallet = Keypair::new();
        self.airdrop(&ncn_fee_wallet.pubkey(), 0.1).await?;

        // Airdroping some SOL to Protocol fee wallet to get it started.
        let jito_fee_wallet = FeeConfig::PROTOCOL_FEE_WALLET;
        self.airdrop(&jito_fee_wallet, 0.1).await?;

        self.initialize_config(
            ncn,
            ncn_admin,
            &ncn_admin_pubkey,
            3,
            10,
            10000,
            &ncn_fee_wallet.pubkey(),
            400,
        )
        .await
    }

    /// Sends a transaction to initialize the NCN config account.
    #[allow(clippy::too_many_arguments)]
    pub async fn initialize_config(
        &mut self,
        ncn: Pubkey,
        ncn_admin: &Keypair,
        tie_breaker_admin: &Pubkey,
        epochs_before_stall: u64,
        epochs_after_consensus_before_close: u64,
        valid_slots_after_consensus: u64,
        ncn_fee_wallet: &Pubkey,
        ncn_fee_bps: u16,
    ) -> TestResult<()> {
        let config = NcnConfig::find_program_address(&ncn_program::id(), &ncn).0;

        let (account_payer, _, _) = AccountPayer::find_program_address(&ncn_program::id(), &ncn);

        let ix = InitializeConfigBuilder::new()
            .config(config)
            .ncn(ncn)
            .ncn_fee_wallet(*ncn_fee_wallet)
            .ncn_admin(ncn_admin.pubkey())
            .account_payer(account_payer)
            .tie_breaker_admin(*tie_breaker_admin)
            .epochs_before_stall(epochs_before_stall)
            .epochs_after_consensus_before_close(epochs_after_consensus_before_close)
            .valid_slots_after_consensus(valid_slots_after_consensus)
            .ncn_fee_bps(ncn_fee_bps)
            .instruction();

        let blockhash = self.banks_client.get_latest_blockhash().await?;
        self.process_transaction(&Transaction::new_signed_with_payer(
            &[ix],
            Some(&ncn_admin.pubkey()),
            &[&ncn_admin],
            blockhash,
        ))
        .await
    }

    /// Sets a new admin for a specific role in the NCN config.
    pub async fn do_set_new_admin(
        &mut self,
        role: ConfigAdminRole,
        new_admin: Pubkey,
        ncn_root: &NcnRoot,
    ) -> TestResult<()> {
        let config_pda =
            NcnConfig::find_program_address(&ncn_program::id(), &ncn_root.ncn_pubkey).0;
        self.airdrop(&ncn_root.ncn_admin.pubkey(), 1.0).await?;
        self.set_new_admin(config_pda, role, new_admin, ncn_root)
            .await
    }

    /// Sends a transaction to set a new admin in the NCN config.
    pub async fn set_new_admin(
        &mut self,
        config_pda: Pubkey,
        role: ConfigAdminRole,
        new_admin: Pubkey,
        ncn_root: &NcnRoot,
    ) -> TestResult<()> {
        let ix = AdminSetNewAdminBuilder::new()
            .config(config_pda)
            .ncn(ncn_root.ncn_pubkey)
            .ncn_admin(ncn_root.ncn_admin.pubkey())
            .new_admin(new_admin)
            .role(role)
            .instruction();

        let blockhash = self.banks_client.get_latest_blockhash().await?;
        self.process_transaction(&Transaction::new_signed_with_payer(
            &[ix],
            Some(&ncn_root.ncn_admin.pubkey()),
            &[&ncn_root.ncn_admin],
            blockhash,
        ))
        .await
    }

    /// Initializes the epoch state account for a given NCN and epoch.
    pub async fn do_intialize_epoch_state(&mut self, ncn: Pubkey, epoch: u64) -> TestResult<()> {
        self.initialize_epoch_state(ncn, epoch).await
    }

    /// Sends a transaction to initialize the epoch state account.
    pub async fn initialize_epoch_state(&mut self, ncn: Pubkey, epoch: u64) -> TestResult<()> {
        let (epoch_marker, _, _) =
            EpochMarker::find_program_address(&ncn_program::id(), &ncn, epoch);
        let epoch_state = EpochState::find_program_address(&ncn_program::id(), &ncn, epoch).0;

        let config = NcnConfig::find_program_address(&ncn_program::id(), &ncn).0;

        let (account_payer, _, _) = AccountPayer::find_program_address(&ncn_program::id(), &ncn);

        let ix = InitializeEpochStateBuilder::new()
            .epoch_marker(epoch_marker)
            .epoch_state(epoch_state)
            .config(config)
            .ncn(ncn)
            .account_payer(account_payer)
            .system_program(system_program::id())
            .epoch(epoch)
            .instruction();

        let blockhash = self.banks_client.get_latest_blockhash().await?;
        self.process_transaction(&Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.payer.pubkey()),
            &[&self.payer],
            blockhash,
        ))
        .await
    }

    /// Initializes and fully reallocates the weight table account for a given NCN and epoch.
    pub async fn do_full_initialize_weight_table(
        &mut self,
        ncn: Pubkey,
        epoch: u64,
    ) -> TestResult<()> {
        self.do_initialize_weight_table(ncn, epoch).await?;
        let num_reallocs = (WeightTable::SIZE as f64 / MAX_REALLOC_BYTES as f64).ceil() as u64 - 1;
        self.do_realloc_weight_table(ncn, epoch, num_reallocs)
            .await?;
        Ok(())
    }

    /// Initializes the weight table account for a given NCN and epoch.
    pub async fn do_initialize_weight_table(&mut self, ncn: Pubkey, epoch: u64) -> TestResult<()> {
        self.initialize_weight_table(ncn, epoch).await
    }

    /// Sends a transaction to initialize the weight table account.
    pub async fn initialize_weight_table(&mut self, ncn: Pubkey, epoch: u64) -> TestResult<()> {
        let (epoch_marker, _, _) =
            EpochMarker::find_program_address(&ncn_program::id(), &ncn, epoch);
        let epoch_state = EpochState::find_program_address(&ncn_program::id(), &ncn, epoch).0;
        let vault_registry = VaultRegistry::find_program_address(&ncn_program::id(), &ncn).0;
        let weight_table = WeightTable::find_program_address(&ncn_program::id(), &ncn, epoch).0;

        let (account_payer, _, _) = AccountPayer::find_program_address(&ncn_program::id(), &ncn);

        let ix = InitializeWeightTableBuilder::new()
            .epoch_marker(epoch_marker)
            .epoch_state(epoch_state)
            .vault_registry(vault_registry)
            .ncn(ncn)
            .weight_table(weight_table)
            .account_payer(account_payer)
            .system_program(system_program::id())
            .epoch(epoch)
            .instruction();

        let blockhash = self.banks_client.get_latest_blockhash().await?;
        self.process_transaction(&Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.payer.pubkey()),
            &[&self.payer],
            blockhash,
        ))
        .await
    }

    /// Sets the epoch weights in the weight table based on the vault registry.
    pub async fn do_set_epoch_weights(&mut self, ncn: Pubkey, epoch: u64) -> TestResult<()> {
        self.set_epoch_weights(ncn, epoch).await
    }

    /// Sends a transaction to set the epoch weights in the weight table.
    pub async fn set_epoch_weights(&mut self, ncn: Pubkey, epoch: u64) -> TestResult<()> {
        let weight_table = WeightTable::find_program_address(&ncn_program::id(), &ncn, epoch).0;
        let epoch_state = EpochState::find_program_address(&ncn_program::id(), &ncn, epoch).0;
        let vault_registry = VaultRegistry::find_program_address(&ncn_program::id(), &ncn).0;

        let ix = SetEpochWeightsBuilder::new()
            .epoch_state(epoch_state)
            .ncn(ncn)
            .weight_table(weight_table)
            .vault_registry(vault_registry)
            .epoch(epoch)
            .instruction();

        let blockhash = self.banks_client.get_latest_blockhash().await?;
        self.process_transaction(&Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.payer.pubkey()),
            &[&self.payer],
            blockhash,
        ))
        .await
    }

    /// Sets the weight for a specific st_mint in the weight table (admin operation).
    pub async fn do_admin_set_weight(
        &mut self,
        ncn: Pubkey,
        epoch: u64,
        st_mint: Pubkey,
        weight: u128,
    ) -> TestResult<()> {
        self.admin_set_weight(ncn, epoch, st_mint, weight).await
    }

    /// Sends a transaction to set the weight for a specific st_mint in the weight table (admin operation).
    pub async fn admin_set_weight(
        &mut self,
        ncn: Pubkey,
        epoch: u64,
        st_mint: Pubkey,
        weight: u128,
    ) -> TestResult<()> {
        let weight_table = WeightTable::find_program_address(&ncn_program::id(), &ncn, epoch).0;
        let epoch_state = EpochState::find_program_address(&ncn_program::id(), &ncn, epoch).0;

        let ix = AdminSetWeightBuilder::new()
            .epoch_state(epoch_state)
            .ncn(ncn)
            .weight_table(weight_table)
            .weight_table_admin(self.payer.pubkey())
            .st_mint(st_mint)
            .weight(weight)
            .epoch(epoch)
            .instruction();

        let blockhash = self.banks_client.get_latest_blockhash().await?;
        self.process_transaction(&Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.payer.pubkey()),
            &[&self.payer],
            blockhash,
        ))
        .await
    }

    /// Initializes and fully reallocates the vault registry account for a given NCN.
    pub async fn do_full_initialize_vault_registry(&mut self, ncn: Pubkey) -> TestResult<()> {
        self.do_initialize_vault_registry(ncn).await?;
        let num_reallocs = (WeightTable::SIZE as f64 / MAX_REALLOC_BYTES as f64).ceil() as u64 - 1;
        self.do_realloc_vault_registry(ncn, num_reallocs).await?;
        Ok(())
    }

    /// Initializes the vault registry account for a given NCN.
    pub async fn do_initialize_vault_registry(&mut self, ncn: Pubkey) -> TestResult<()> {
        let ncn_config = NcnConfig::find_program_address(&ncn_program::id(), &ncn).0;
        let vault_registry = VaultRegistry::find_program_address(&ncn_program::id(), &ncn).0;

        self.initialize_vault_registry(&ncn_config, &vault_registry, &ncn)
            .await
    }

    /// Sends a transaction to initialize the vault registry account.
    pub async fn initialize_vault_registry(
        &mut self,
        ncn_config: &Pubkey,
        vault_registry: &Pubkey,
        ncn: &Pubkey,
    ) -> TestResult<()> {
        let (account_payer, _, _) = AccountPayer::find_program_address(&ncn_program::id(), ncn);

        let ix = InitializeVaultRegistryBuilder::new()
            .config(*ncn_config)
            .vault_registry(*vault_registry)
            .ncn(*ncn)
            .account_payer(account_payer)
            .system_program(system_program::id())
            .instruction();

        let blockhash = self.banks_client.get_latest_blockhash().await?;
        self.process_transaction(&Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.payer.pubkey()),
            &[&self.payer],
            blockhash,
        ))
        .await
    }

    /// Reallocates the vault registry account multiple times.
    pub async fn do_realloc_vault_registry(
        &mut self,
        ncn: Pubkey,
        num_reallocations: u64,
    ) -> TestResult<()> {
        let ncn_config = NcnConfig::find_program_address(&ncn_program::id(), &ncn).0;
        let vault_registry = VaultRegistry::find_program_address(&ncn_program::id(), &ncn).0;
        self.realloc_vault_registry(&ncn, &ncn_config, &vault_registry, num_reallocations)
            .await
    }

    /// Sends transactions to reallocate the vault registry account.
    pub async fn realloc_vault_registry(
        &mut self,
        ncn: &Pubkey,
        config: &Pubkey,
        vault_registry: &Pubkey,
        num_reallocations: u64,
    ) -> TestResult<()> {
        let (account_payer, _, _) = AccountPayer::find_program_address(&ncn_program::id(), ncn);

        let ix = ReallocVaultRegistryBuilder::new()
            .ncn(*ncn)
            .account_payer(account_payer)
            .config(*config)
            .vault_registry(*vault_registry)
            .system_program(system_program::id())
            .instruction();

        let ixs = vec![ix; num_reallocations as usize];

        let blockhash = self.banks_client.get_latest_blockhash().await?;
        self.process_transaction(&Transaction::new_signed_with_payer(
            &ixs,
            Some(&self.payer.pubkey()),
            &[&self.payer],
            blockhash,
        ))
        .await
    }

    /// Registers a vault with the NCN program.
    pub async fn do_register_vault(
        &mut self,
        ncn: Pubkey,
        vault: Pubkey,
        ncn_vault_ticket: Pubkey,
    ) -> TestResult<()> {
        let ncn_config = NcnConfig::find_program_address(&ncn_program::id(), &ncn).0;

        let vault_registry = VaultRegistry::find_program_address(&ncn_program::id(), &ncn).0;

        self.register_vault(ncn_config, vault_registry, ncn, vault, ncn_vault_ticket)
            .await
    }

    /// Sends a transaction to register a vault.
    pub async fn register_vault(
        &mut self,
        config: Pubkey,
        vault_registry: Pubkey,
        ncn: Pubkey,
        vault: Pubkey,
        ncn_vault_ticket: Pubkey,
    ) -> TestResult<()> {
        let ix = RegisterVaultBuilder::new()
            .config(config)
            .vault_registry(vault_registry)
            .ncn(ncn)
            .vault(vault)
            .ncn_vault_ticket(ncn_vault_ticket)
            .instruction();

        let blockhash = self.banks_client.get_latest_blockhash().await?;
        self.process_transaction(&Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.payer.pubkey()),
            &[&self.payer],
            blockhash,
        ))
        .await
    }

    /// Registers an st_mint with a specific weight in the vault registry (admin operation).
    pub async fn do_admin_register_st_mint(
        &mut self,
        ncn: Pubkey,
        st_mint: Pubkey,
        weight: u128,
    ) -> TestResult<()> {
        let vault_registry = VaultRegistry::find_program_address(&ncn_program::id(), &ncn).0;

        let (ncn_config, _, _) = NcnConfig::find_program_address(&ncn_program::id(), &ncn);

        let admin = self.payer.pubkey();

        self.admin_register_st_mint(ncn, ncn_config, vault_registry, admin, st_mint, weight)
            .await
    }

    /// Sends a transaction to register an st_mint in the vault registry (admin operation).
    #[allow(clippy::too_many_arguments)]
    pub async fn admin_register_st_mint(
        &mut self,
        ncn: Pubkey,
        ncn_config: Pubkey,
        vault_registry: Pubkey,
        admin: Pubkey,
        st_mint: Pubkey,
        weight: u128,
    ) -> TestResult<()> {
        let ix = {
            let mut builder = AdminRegisterStMintBuilder::new();
            builder
                .config(ncn_config)
                .ncn(ncn)
                .vault_registry(vault_registry)
                .admin(admin)
                .st_mint(st_mint)
                .weight(weight);

            builder.instruction()
        };

        let blockhash = self.banks_client.get_latest_blockhash().await?;
        self.process_transaction(&Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.payer.pubkey()),
            &[&self.payer],
            blockhash,
        ))
        .await
    }

    /// Sets the weight for an existing st_mint in the vault registry (admin operation).
    pub async fn do_admin_set_st_mint(
        &mut self,
        ncn: Pubkey,
        st_mint: Pubkey,
        weight: u128,
    ) -> TestResult<()> {
        let vault_registry = VaultRegistry::find_program_address(&ncn_program::id(), &ncn).0;

        let (ncn_config, _, _) = NcnConfig::find_program_address(&ncn_program::id(), &ncn);

        let admin = self.payer.pubkey();

        self.admin_set_st_mint(ncn, ncn_config, vault_registry, admin, st_mint, weight)
            .await
    }

    /// Sends a transaction to set the weight for an st_mint in the vault registry (admin operation).
    #[allow(clippy::too_many_arguments)]
    pub async fn admin_set_st_mint(
        &mut self,
        ncn: Pubkey,
        ncn_config: Pubkey,
        vault_registry: Pubkey,
        admin: Pubkey,
        st_mint: Pubkey,
        weight: u128,
    ) -> TestResult<()> {
        let ix = {
            let mut builder = AdminSetStMintBuilder::new();
            builder
                .config(ncn_config)
                .ncn(ncn)
                .vault_registry(vault_registry)
                .admin(admin)
                .st_mint(st_mint)
                .weight(weight);

            builder.instruction()
        };

        let blockhash = self.banks_client.get_latest_blockhash().await?;
        self.process_transaction(&Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.payer.pubkey()),
            &[&self.payer],
            blockhash,
        ))
        .await
    }

    /// Initializes the epoch snapshot account for a given NCN and epoch.
    pub async fn do_initialize_epoch_snapshot(
        &mut self,
        ncn: Pubkey,
        epoch: u64,
    ) -> TestResult<()> {
        self.initialize_epoch_snapshot(ncn, epoch).await
    }

    /// Sends a transaction to initialize the epoch snapshot account.
    pub async fn initialize_epoch_snapshot(&mut self, ncn: Pubkey, epoch: u64) -> TestResult<()> {
        let config_pda = NcnConfig::find_program_address(&ncn_program::id(), &ncn).0;
        let (epoch_marker, _, _) =
            EpochMarker::find_program_address(&ncn_program::id(), &ncn, epoch);
        let epoch_state = EpochState::find_program_address(&ncn_program::id(), &ncn, epoch).0;
        let weight_table = WeightTable::find_program_address(&ncn_program::id(), &ncn, epoch).0;
        let epoch_snapshot = EpochSnapshot::find_program_address(&ncn_program::id(), &ncn, epoch).0;

        let (account_payer, _, _) = AccountPayer::find_program_address(&ncn_program::id(), &ncn);

        let ix = InitializeEpochSnapshotBuilder::new()
            .epoch_marker(epoch_marker)
            .epoch_state(epoch_state)
            .config(config_pda)
            .ncn(ncn)
            .weight_table(weight_table)
            .epoch_snapshot(epoch_snapshot)
            .account_payer(account_payer)
            .system_program(system_program::id())
            .epoch(epoch)
            .instruction();

        let blockhash = self.banks_client.get_latest_blockhash().await?;
        self.process_transaction(&Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.payer.pubkey()),
            &[&self.payer],
            blockhash,
        ))
        .await
    }

    /// Initializes the operator snapshot account for a given operator, NCN, and epoch.
    pub async fn do_initialize_operator_snapshot(
        &mut self,
        operator: Pubkey,
        ncn: Pubkey,
        epoch: u64,
    ) -> TestResult<()> {
        self.initialize_operator_snapshot(operator, ncn, epoch)
            .await
    }

    /// Sends a transaction to initialize the operator snapshot account.
    pub async fn initialize_operator_snapshot(
        &mut self,
        operator: Pubkey,
        ncn: Pubkey,
        epoch: u64,
    ) -> TestResult<()> {
        let (epoch_marker, _, _) =
            EpochMarker::find_program_address(&ncn_program::id(), &ncn, epoch);
        let epoch_state = EpochState::find_program_address(&ncn_program::id(), &ncn, epoch).0;
        let config_pda = NcnConfig::find_program_address(&ncn_program::id(), &ncn).0;
        let ncn_operator_state =
            NcnOperatorState::find_program_address(&jito_restaking_program::id(), &ncn, &operator)
                .0;
        let epoch_snapshot = EpochSnapshot::find_program_address(&ncn_program::id(), &ncn, epoch).0;
        let operator_snapshot =
            OperatorSnapshot::find_program_address(&ncn_program::id(), &operator, &ncn, epoch).0;

        let (account_payer, _, _) = AccountPayer::find_program_address(&ncn_program::id(), &ncn);

        let restaking_config = Config::find_program_address(&jito_restaking_program::id()).0;

        let ix = InitializeOperatorSnapshotBuilder::new()
            .epoch_marker(epoch_marker)
            .epoch_state(epoch_state)
            .config(config_pda)
            .restaking_config(restaking_config)
            .ncn(ncn)
            .operator(operator)
            .ncn_operator_state(ncn_operator_state)
            .epoch_snapshot(epoch_snapshot)
            .operator_snapshot(operator_snapshot)
            .account_payer(account_payer)
            .system_program(system_program::id())
            .epoch(epoch)
            .instruction();

        let blockhash = self.banks_client.get_latest_blockhash().await?;
        self.process_transaction(&Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.payer.pubkey()),
            &[&self.payer],
            blockhash,
        ))
        .await
    }

    /// Snapshots the delegation information from a vault to an operator for a given NCN and epoch.
    pub async fn do_snapshot_vault_operator_delegation(
        &mut self,
        vault: Pubkey,
        operator: Pubkey,
        ncn: Pubkey,
        epoch: u64,
    ) -> TestResult<()> {
        self.snapshot_vault_operator_delegation(vault, operator, ncn, epoch)
            .await
    }

    /// Sends a transaction to snapshot the vault operator delegation.
    pub async fn snapshot_vault_operator_delegation(
        &mut self,
        vault: Pubkey,
        operator: Pubkey,
        ncn: Pubkey,
        epoch: u64,
    ) -> TestResult<()> {
        let epoch_state = EpochState::find_program_address(&ncn_program::id(), &ncn, epoch).0;
        let restaking_config = Config::find_program_address(&jito_restaking_program::id()).0;

        let config_pda = NcnConfig::find_program_address(&ncn_program::id(), &ncn).0;

        let epoch_snapshot = EpochSnapshot::find_program_address(&ncn_program::id(), &ncn, epoch).0;
        let operator_snapshot =
            OperatorSnapshot::find_program_address(&ncn_program::id(), &operator, &ncn, epoch).0;

        let vault_ncn_ticket =
            VaultNcnTicket::find_program_address(&jito_vault_program::id(), &vault, &ncn).0;

        let ncn_vault_ticket =
            NcnVaultTicket::find_program_address(&jito_restaking_program::id(), &ncn, &vault).0;

        let vault_operator_delegation = VaultOperatorDelegation::find_program_address(
            &jito_vault_program::id(),
            &vault,
            &operator,
        )
        .0;

        let weight_table = WeightTable::find_program_address(&ncn_program::id(), &ncn, epoch).0;

        let ix = SnapshotVaultOperatorDelegationBuilder::new()
            .epoch_state(epoch_state)
            .config(config_pda)
            .restaking_config(restaking_config)
            .ncn(ncn)
            .operator(operator)
            .vault(vault)
            .vault_ncn_ticket(vault_ncn_ticket)
            .ncn_vault_ticket(ncn_vault_ticket)
            .vault_operator_delegation(vault_operator_delegation)
            .weight_table(weight_table)
            .epoch_snapshot(epoch_snapshot)
            .operator_snapshot(operator_snapshot)
            .epoch(epoch)
            .instruction();

        let blockhash = self.banks_client.get_latest_blockhash().await?;
        self.process_transaction(&Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.payer.pubkey()),
            &[&self.payer],
            blockhash,
        ))
        .await
    }

    /// Initializes and fully reallocates the ballot box account for a given NCN and epoch.
    pub async fn do_full_initialize_ballot_box(
        &mut self,
        ncn: Pubkey,
        epoch: u64,
    ) -> TestResult<()> {
        self.do_initialize_ballot_box(ncn, epoch).await?;
        let num_reallocs = (BallotBox::SIZE as f64 / MAX_REALLOC_BYTES as f64).ceil() as u64 - 1;
        self.do_realloc_ballot_box(ncn, epoch, num_reallocs).await?;
        Ok(())
    }

    /// Initializes the ballot box account for a given NCN and epoch.
    pub async fn do_initialize_ballot_box(
        &mut self,
        ncn: Pubkey,
        epoch: u64,
    ) -> Result<(), TestError> {
        let ncn_config = NcnConfig::find_program_address(&ncn_program::id(), &ncn).0;

        let ballot_box = ncn_program_core::ballot_box::BallotBox::find_program_address(
            &ncn_program::id(),
            &ncn,
            epoch,
        )
        .0;

        self.initialize_ballot_box(ncn_config, ballot_box, ncn, epoch)
            .await
    }

    /// Sends a transaction to initialize the ballot box account.
    pub async fn initialize_ballot_box(
        &mut self,
        config: Pubkey,
        ballot_box: Pubkey,
        ncn: Pubkey,
        epoch: u64,
    ) -> Result<(), TestError> {
        let (epoch_marker, _, _) =
            EpochMarker::find_program_address(&ncn_program::id(), &ncn, epoch);
        let epoch_state = EpochState::find_program_address(&ncn_program::id(), &ncn, epoch).0;

        let (account_payer, _, _) = AccountPayer::find_program_address(&ncn_program::id(), &ncn);

        let (consensus_result, _, _) =
            ConsensusResult::find_program_address(&ncn_program::id(), &ncn, epoch);

        let ix = InitializeBallotBoxBuilder::new()
            .epoch_marker(epoch_marker)
            .epoch_state(epoch_state)
            .config(config)
            .ballot_box(ballot_box)
            .ncn(ncn)
            .epoch(epoch)
            .account_payer(account_payer)
            .consensus_result(consensus_result)
            .instruction();

        let compute_budget_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_000_000);

        let blockhash = self.banks_client.get_latest_blockhash().await?;
        self.process_transaction(&Transaction::new_signed_with_payer(
            &[compute_budget_ix, ix],
            Some(&self.payer.pubkey()),
            &[&self.payer],
            blockhash,
        ))
        .await
    }

    /// Reallocates the ballot box account multiple times.
    pub async fn do_realloc_ballot_box(
        &mut self,
        ncn: Pubkey,
        epoch: u64,
        num_reallocations: u64,
    ) -> Result<(), TestError> {
        let ncn_config = NcnConfig::find_program_address(&ncn_program::id(), &ncn).0;

        let ballot_box = ncn_program_core::ballot_box::BallotBox::find_program_address(
            &ncn_program::id(),
            &ncn,
            epoch,
        )
        .0;

        self.realloc_ballot_box(ncn_config, ballot_box, ncn, epoch, num_reallocations)
            .await
    }

    /// Sends transactions to reallocate the ballot box account.
    pub async fn realloc_ballot_box(
        &mut self,
        config: Pubkey,
        ballot_box: Pubkey,
        ncn: Pubkey,
        epoch: u64,
        num_reallocations: u64,
    ) -> Result<(), TestError> {
        let epoch_state = EpochState::find_program_address(&ncn_program::id(), &ncn, epoch).0;

        let (account_payer, _, _) = AccountPayer::find_program_address(&ncn_program::id(), &ncn);

        let ix = ReallocBallotBoxBuilder::new()
            .epoch_state(epoch_state)
            .config(config)
            .ballot_box(ballot_box)
            .ncn(ncn)
            .epoch(epoch)
            .account_payer(account_payer)
            .instruction();

        let ixs = vec![ix; num_reallocations as usize];

        let blockhash = self.banks_client.get_latest_blockhash().await?;
        self.process_transaction(&Transaction::new_signed_with_payer(
            &ixs,
            Some(&self.payer.pubkey()),
            &[&self.payer],
            blockhash,
        ))
        .await
    }

    /// Casts a vote for a given operator in a specific epoch.
    pub async fn do_cast_vote(
        &mut self,
        ncn: Pubkey,
        operator: Pubkey,
        operator_admin: &Keypair,
        weather_status: u8,
        epoch: u64,
    ) -> Result<(), TestError> {
        let ncn_config = NcnConfig::find_program_address(&ncn_program::id(), &ncn).0;

        let ballot_box = ncn_program_core::ballot_box::BallotBox::find_program_address(
            &ncn_program::id(),
            &ncn,
            epoch,
        )
        .0;

        let epoch_snapshot = ncn_program_core::epoch_snapshot::EpochSnapshot::find_program_address(
            &ncn_program::id(),
            &ncn,
            epoch,
        )
        .0;

        let operator_snapshot =
            ncn_program_core::epoch_snapshot::OperatorSnapshot::find_program_address(
                &ncn_program::id(),
                &operator,
                &ncn,
                epoch,
            )
            .0;

        self.cast_vote(
            ncn_config,
            ballot_box,
            ncn,
            epoch_snapshot,
            operator_snapshot,
            operator,
            operator_admin,
            weather_status,
            epoch,
        )
        .await
    }

    /// Sends a transaction to cast a vote.
    #[allow(clippy::too_many_arguments)]
    pub async fn cast_vote(
        &mut self,
        ncn_config: Pubkey,
        ballot_box: Pubkey,
        ncn: Pubkey,
        epoch_snapshot: Pubkey,
        operator_snapshot: Pubkey,
        operator: Pubkey,
        operator_voter: &Keypair,
        weather_status: u8,
        epoch: u64,
    ) -> Result<(), TestError> {
        let epoch_state = EpochState::find_program_address(&ncn_program::id(), &ncn, epoch).0;
        let consensus_result =
            ConsensusResult::find_program_address(&ncn_program::id(), &ncn, epoch).0;

        let compute_budget_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_000_000);

        let ix = CastVoteBuilder::new()
            .epoch_state(epoch_state)
            .config(ncn_config)
            .ballot_box(ballot_box)
            .ncn(ncn)
            .epoch_snapshot(epoch_snapshot)
            .operator_snapshot(operator_snapshot)
            .operator(operator)
            .operator_voter(operator_voter.pubkey())
            .weather_status(weather_status)
            .consensus_result(consensus_result)
            .epoch(epoch)
            .instruction();

        let blockhash = self.banks_client.get_latest_blockhash().await?;
        self.process_transaction(&Transaction::new_signed_with_payer(
            &[compute_budget_ix, ix],
            Some(&self.payer.pubkey()),
            &[&self.payer, operator_voter],
            blockhash,
        ))
        .await
    }

    /// Sets the tie-breaker weather status for an epoch (admin operation).
    pub async fn do_admin_set_tie_breaker(
        &mut self,
        ncn: Pubkey,
        weather_status: u8,
        epoch: u64,
    ) -> Result<(), TestError> {
        let ncn_config = NcnConfig::find_program_address(&ncn_program::id(), &ncn).0;
        let ballot_box = BallotBox::find_program_address(&ncn_program::id(), &ncn, epoch).0;

        let tie_breaker_admin = self.payer.pubkey();

        self.admin_set_tie_breaker(
            ncn_config,
            ballot_box,
            ncn,
            tie_breaker_admin,
            weather_status,
            epoch,
        )
        .await
    }

    /// Sends a transaction to set the tie-breaker weather status (admin operation).
    pub async fn admin_set_tie_breaker(
        &mut self,
        ncn_config: Pubkey,
        ballot_box: Pubkey,
        ncn: Pubkey,
        tie_breaker_admin: Pubkey,
        weather_status: u8,
        epoch: u64,
    ) -> Result<(), TestError> {
        let epoch_state = EpochState::find_program_address(&ncn_program::id(), &ncn, epoch).0;

        let ix = AdminSetTieBreakerBuilder::new()
            .epoch_state(epoch_state)
            .config(ncn_config)
            .ballot_box(ballot_box)
            .ncn(ncn)
            .tie_breaker_admin(tie_breaker_admin)
            .weather_status(weather_status)
            .epoch(epoch)
            .instruction();

        let blockhash = self.banks_client.get_latest_blockhash().await?;
        self.process_transaction(&Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.payer.pubkey()),
            &[&self.payer],
            blockhash,
        ))
        .await
    }

    /// Reallocates the weight table account multiple times.
    pub async fn do_realloc_weight_table(
        &mut self,
        ncn: Pubkey,
        epoch: u64,
        num_reallocations: u64,
    ) -> Result<(), TestError> {
        let ncn_config = NcnConfig::find_program_address(&ncn_program::id(), &ncn).0;
        let weight_table = WeightTable::find_program_address(&ncn_program::id(), &ncn, epoch).0;
        let vault_registry = VaultRegistry::find_program_address(&ncn_program::id(), &ncn).0;

        self.realloc_weight_table(
            ncn_config,
            weight_table,
            ncn,
            vault_registry,
            epoch,
            num_reallocations,
        )
        .await
    }

    /// Sends transactions to reallocate the weight table account.
    pub async fn realloc_weight_table(
        &mut self,
        ncn_config: Pubkey,
        weight_table: Pubkey,
        ncn: Pubkey,
        vault_registry: Pubkey,
        epoch: u64,
        num_reallocations: u64,
    ) -> Result<(), TestError> {
        let epoch_state = EpochState::find_program_address(&ncn_program::id(), &ncn, epoch).0;

        let (account_payer, _, _) = AccountPayer::find_program_address(&ncn_program::id(), &ncn);

        let ix = ReallocWeightTableBuilder::new()
            .epoch_state(epoch_state)
            .config(ncn_config)
            .weight_table(weight_table)
            .ncn(ncn)
            .vault_registry(vault_registry)
            .epoch(epoch)
            .account_payer(account_payer)
            .system_program(system_program::id())
            .instruction();

        let ixs = vec![ix; num_reallocations as usize];

        let blockhash = self.banks_client.get_latest_blockhash().await?;
        self.process_transaction(&Transaction::new_signed_with_payer(
            &ixs,
            Some(&self.payer.pubkey()),
            &[&self.payer],
            blockhash,
        ))
        .await
    }

    /// Closes an epoch-specific account (e.g., BallotBox, EpochSnapshot) after the epoch is finished.
    pub async fn do_close_epoch_account(
        &mut self,
        ncn: Pubkey,
        epoch: u64,
        account_to_close: Pubkey,
    ) -> TestResult<()> {
        let (epoch_marker, _, _) =
            EpochMarker::find_program_address(&ncn_program::id(), &ncn, epoch);

        let epoch_state = EpochState::find_program_address(&ncn_program::id(), &ncn, epoch).0;

        let (account_payer, _, _) = AccountPayer::find_program_address(&ncn_program::id(), &ncn);

        let (config, _, _) = NcnConfig::find_program_address(&ncn_program::id(), &ncn);

        self.close_epoch_account(
            epoch_marker,
            epoch_state,
            ncn,
            config,
            account_to_close,
            account_payer,
            None,
            None,
            epoch,
        )
        .await
    }

    pub async fn do_close_router_epoch_account(
        &mut self,
        ncn: Pubkey,
        epoch: u64,
        account_to_close: Pubkey,
        receiver_to_close: Pubkey,
    ) -> TestResult<()> {
        let (epoch_marker, _, _) =
            EpochMarker::find_program_address(&ncn_program::id(), &ncn, epoch);

        let epoch_state = EpochState::find_program_address(&ncn_program::id(), &ncn, epoch).0;

        let (account_payer, _, _) = AccountPayer::find_program_address(&ncn_program::id(), &ncn);

        let (config, _, _) = NcnConfig::find_program_address(&ncn_program::id(), &ncn);

        let config_account = self.get_ncn_config(ncn).await?;
        let ncn_fee_wallet = *config_account.fee_config.ncn_fee_wallet();

        self.close_epoch_account(
            epoch_marker,
            epoch_state,
            ncn,
            config,
            account_to_close,
            account_payer,
            Some(receiver_to_close),
            Some(ncn_fee_wallet),
            epoch,
        )
        .await
    }

    /// Sends a transaction to close an epoch-specific account.
    #[allow(clippy::too_many_arguments)]
    pub async fn close_epoch_account(
        &mut self,
        epoch_marker: Pubkey,
        epoch_state: Pubkey,
        ncn: Pubkey,
        config: Pubkey,
        account_to_close: Pubkey,
        account_payer: Pubkey,

        receiver_to_close: Option<Pubkey>,
        ncn_fee_wallet: Option<Pubkey>,

        epoch: u64,
    ) -> TestResult<()> {
        let mut ix = CloseEpochAccountBuilder::new();

        ix.account_payer(account_payer)
            .epoch_marker(epoch_marker)
            .config(config)
            .account_to_close(account_to_close)
            .epoch_state(epoch_state)
            .ncn(ncn)
            .system_program(system_program::id())
            .epoch(epoch);

        if let Some(receiver_to_close) = receiver_to_close {
            ix.receiver_to_close(Some(receiver_to_close));
        }

        if let Some(ncn_fee_wallet) = ncn_fee_wallet {
            ix.ncn_fee_wallet(Some(ncn_fee_wallet));
        }

        let ix = ix.instruction();

        let blockhash = self.banks_client.get_latest_blockhash().await?;
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.payer.pubkey()),
            &[&self.payer],
            blockhash,
        );

        self.process_transaction(&tx).await
    }

    /// Sets various parameters in the NCN config (admin operation).
    pub async fn do_set_parameters(
        &mut self,
        starting_valid_epoch: Option<u64>,
        epochs_before_stall: Option<u64>,
        epochs_after_consensus_before_close: Option<u64>,
        valid_slots_after_consensus: Option<u64>,
        ncn_root: &NcnRoot,
    ) -> TestResult<()> {
        let config_pda =
            NcnConfig::find_program_address(&ncn_program::id(), &ncn_root.ncn_pubkey).0;

        let mut ix = AdminSetParametersBuilder::new();
        ix.config(config_pda)
            .ncn(ncn_root.ncn_pubkey)
            .ncn_admin(ncn_root.ncn_admin.pubkey());

        if let Some(epoch) = starting_valid_epoch {
            ix.starting_valid_epoch(epoch);
        }

        if let Some(epochs) = epochs_before_stall {
            ix.epochs_before_stall(epochs);
        }

        if let Some(epochs) = epochs_after_consensus_before_close {
            ix.epochs_after_consensus_before_close(epochs);
        }

        if let Some(slots) = valid_slots_after_consensus {
            ix.valid_slots_after_consensus(slots);
        }

        let blockhash = self.banks_client.get_latest_blockhash().await?;
        self.process_transaction(&Transaction::new_signed_with_payer(
            &[ix.instruction()],
            Some(&ncn_root.ncn_admin.pubkey()),
            &[&ncn_root.ncn_admin],
            blockhash,
        ))
        .await
    }

    pub async fn get_ncn_reward_router(
        &mut self,
        ncn: Pubkey,
        ncn_epoch: u64,
    ) -> TestResult<NCNRewardRouter> {
        let address = NCNRewardRouter::find_program_address(&ncn_program::id(), &ncn, ncn_epoch).0;

        let raw_account = self.banks_client.get_account(address).await?.unwrap();

        let account =
            NCNRewardRouter::try_from_slice_unchecked(raw_account.data.as_slice()).unwrap();
        Ok(*account)
    }

    pub async fn get_operator_vault_reward_router(
        &mut self,
        operator: Pubkey,
        ncn: Pubkey,
        epoch: u64,
    ) -> TestResult<OperatorVaultRewardRouter> {
        let address = OperatorVaultRewardRouter::find_program_address(
            &ncn_program::id(),
            &operator,
            &ncn,
            epoch,
        )
        .0;

        let raw_account = self.banks_client.get_account(address).await?.unwrap();

        let account =
            OperatorVaultRewardRouter::try_from_slice_unchecked(raw_account.data.as_slice())
                .unwrap();

        Ok(*account)
    }

    #[allow(dead_code)]
    pub async fn log_all_operator_vault_reward_routers(
        &mut self,
        operators: Vec<Pubkey>,
        ncn: Pubkey,
        epoch: u64,
    ) -> TestResult<()> {
        msg!(
            "Logging all operator vault reward routers for epoch {}",
            epoch
        );
        msg!("------------------------------------------------------------------------");
        for operator in operators {
            let operator_vault_reward_router = self
                .get_operator_vault_reward_router(operator, ncn, epoch)
                .await?;
            msg!(
                "Operator vault reward router for operator {}: {}",
                operator,
                operator_vault_reward_router
            );
        }
        Ok(())
    }

    pub async fn do_full_initialize_ncn_reward_router(
        &mut self,
        ncn: Pubkey,
        epoch: u64,
    ) -> TestResult<()> {
        self.do_initialize_ncn_reward_router(ncn, epoch).await?;
        let num_reallocs =
            (NCNRewardRouter::SIZE as f64 / MAX_REALLOC_BYTES as f64).ceil() as u64 - 1;
        self.do_realloc_ncn_reward_router(ncn, epoch, num_reallocs)
            .await?;
        Ok(())
    }

    pub async fn do_initialize_ncn_reward_router(
        &mut self,
        ncn: Pubkey,
        epoch: u64,
    ) -> TestResult<()> {
        let (ncn_reward_router, _, _) =
            NCNRewardRouter::find_program_address(&ncn_program::id(), &ncn, epoch);

        let (ncn_reward_receiver, _, _) =
            NCNRewardReceiver::find_program_address(&ncn_program::id(), &ncn, epoch);

        self.initialize_ncn_reward_router(ncn, ncn_reward_router, ncn_reward_receiver, epoch)
            .await
    }

    pub async fn initialize_ncn_reward_router(
        &mut self,
        ncn: Pubkey,
        ncn_reward_router: Pubkey,
        ncn_reward_receiver: Pubkey,
        epoch: u64,
    ) -> TestResult<()> {
        let (epoch_marker, _, _) =
            EpochMarker::find_program_address(&ncn_program::id(), &ncn, epoch);
        let epoch_state = EpochState::find_program_address(&ncn_program::id(), &ncn, epoch).0;

        let (account_payer, _, _) = AccountPayer::find_program_address(&ncn_program::id(), &ncn);

        let ix = InitializeNCNRewardRouterBuilder::new()
            .epoch_marker(epoch_marker)
            .epoch_state(epoch_state)
            .ncn(ncn)
            .ncn_reward_router(ncn_reward_router)
            .ncn_reward_receiver(ncn_reward_receiver)
            .account_payer(account_payer)
            .system_program(system_program::id())
            .epoch(epoch)
            .instruction();

        let blockhash = self.banks_client.get_latest_blockhash().await?;
        self.process_transaction(&Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.payer.pubkey()),
            &[&self.payer],
            blockhash,
        ))
        .await
    }

    pub async fn do_realloc_ncn_reward_router(
        &mut self,
        ncn: Pubkey,
        epoch: u64,
        num_reallocations: u64,
    ) -> Result<(), TestError> {
        let ncn_config = NcnConfig::find_program_address(&ncn_program::id(), &ncn).0;
        let ncn_reward_router =
            NCNRewardRouter::find_program_address(&ncn_program::id(), &ncn, epoch).0;

        self.realloc_ncn_reward_router(ncn_config, ncn_reward_router, ncn, epoch, num_reallocations)
            .await
    }

    pub async fn realloc_ncn_reward_router(
        &mut self,
        ncn_config: Pubkey,
        ncn_reward_router: Pubkey,
        ncn: Pubkey,
        epoch: u64,
        num_reallocations: u64,
    ) -> Result<(), TestError> {
        let epoch_state = EpochState::find_program_address(&ncn_program::id(), &ncn, epoch).0;

        let (account_payer, _, _) = AccountPayer::find_program_address(&ncn_program::id(), &ncn);

        let ix = ReallocNCNRewardRouterBuilder::new()
            .epoch_state(epoch_state)
            .config(ncn_config)
            .ncn_reward_router(ncn_reward_router)
            .ncn(ncn)
            .epoch(epoch)
            .account_payer(account_payer)
            .system_program(system_program::id())
            .instruction();

        let ixs = vec![ix; num_reallocations as usize];

        let blockhash = self.banks_client.get_latest_blockhash().await?;
        self.process_transaction(&Transaction::new_signed_with_payer(
            &ixs,
            Some(&self.payer.pubkey()),
            &[&self.payer],
            blockhash,
        ))
        .await
    }

    pub async fn do_initialize_operator_vault_reward_router(
        &mut self,
        ncn: Pubkey,
        operator: Pubkey,
        epoch: u64,
    ) -> TestResult<()> {
        let (operator_vault_reward_router, _, _) = OperatorVaultRewardRouter::find_program_address(
            &ncn_program::id(),
            &operator,
            &ncn,
            epoch,
        );

        let (operator_vault_reward_receiver, _, _) =
            OperatorVaultRewardReceiver::find_program_address(
                &ncn_program::id(),
                &operator,
                &ncn,
                epoch,
            );

        let (operator_snapshot, _, _) =
            OperatorSnapshot::find_program_address(&ncn_program::id(), &operator, &ncn, epoch);

        self.initialize_operator_vault_reward_router(
            ncn,
            operator,
            operator_snapshot,
            operator_vault_reward_router,
            operator_vault_reward_receiver,
            epoch,
        )
        .await
    }

    pub async fn initialize_operator_vault_reward_router(
        &mut self,
        ncn: Pubkey,
        operator: Pubkey,
        operator_snapshot: Pubkey,
        operator_vault_reward_router: Pubkey,
        operator_vault_reward_receiver: Pubkey,
        epoch: u64,
    ) -> TestResult<()> {
        let (epoch_marker, _, _) =
            EpochMarker::find_program_address(&ncn_program::id(), &ncn, epoch);

        let epoch_state = EpochState::find_program_address(&ncn_program::id(), &ncn, epoch).0;

        let (account_payer, _, _) = AccountPayer::find_program_address(&ncn_program::id(), &ncn);

        let ix = InitializeOperatorVaultRewardRouterBuilder::new()
            .epoch_marker(epoch_marker)
            .epoch_state(epoch_state)
            .ncn(ncn)
            .operator(operator)
            .operator_snapshot(operator_snapshot)
            .operator_vault_reward_router(operator_vault_reward_router)
            .operator_vault_reward_receiver(operator_vault_reward_receiver)
            .account_payer(account_payer)
            .system_program(system_program::id())
            .epoch(epoch)
            .instruction();

        let blockhash = self.banks_client.get_latest_blockhash().await?;
        self.process_transaction(&Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.payer.pubkey()),
            &[&self.payer],
            blockhash,
        ))
        .await
    }

    pub async fn do_route_ncn_rewards(&mut self, ncn: Pubkey, epoch: u64) -> TestResult<()> {
        let (epoch_snapshot, _, _) =
            EpochSnapshot::find_program_address(&ncn_program::id(), &ncn, epoch);

        let (ballot_box, _, _) = BallotBox::find_program_address(&ncn_program::id(), &ncn, epoch);

        let (ncn_reward_router, _, _) =
            NCNRewardRouter::find_program_address(&ncn_program::id(), &ncn, epoch);

        let (ncn_reward_receiver, _, _) =
            NCNRewardReceiver::find_program_address(&ncn_program::id(), &ncn, epoch);

        //Pretty close to max
        let max_iterations: u16 = NCNRewardRouter::MAX_ROUTE_BASE_ITERATIONS;

        let mut still_routing = true;
        while still_routing {
            self.route_ncn_rewards(
                ncn,
                epoch_snapshot,
                ballot_box,
                ncn_reward_router,
                ncn_reward_receiver,
                max_iterations,
                epoch,
            )
            .await?;

            let ncn_reward_router_account = self.get_ncn_reward_router(ncn, epoch).await?;

            still_routing = ncn_reward_router_account.still_routing();
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn route_ncn_rewards(
        &mut self,
        ncn: Pubkey,
        epoch_snapshot: Pubkey,
        ballot_box: Pubkey,
        ncn_reward_router: Pubkey,
        ncn_reward_receiver: Pubkey,
        max_iterations: u16,
        epoch: u64,
    ) -> TestResult<()> {
        let epoch_state = EpochState::find_program_address(&ncn_program::id(), &ncn, epoch).0;

        let config = NcnConfig::find_program_address(&ncn_program::id(), &ncn).0;

        let ix = RouteNCNRewardsBuilder::new()
            .epoch_state(epoch_state)
            .config(config)
            .ncn(ncn)
            .epoch_snapshot(epoch_snapshot)
            .ballot_box(ballot_box)
            .ncn_reward_router(ncn_reward_router)
            .ncn_reward_receiver(ncn_reward_receiver)
            .max_iterations(max_iterations)
            .epoch(epoch)
            .instruction();

        let blockhash = self.get_best_latest_blockhash().await?;
        let tx = &Transaction::new_signed_with_payer(
            &[
                ComputeBudgetInstruction::set_compute_unit_limit(1_400_000),
                ix,
            ],
            Some(&self.payer.pubkey()),
            &[&self.payer],
            blockhash,
        );

        self.process_transaction(tx).await
    }

    pub async fn do_distribute_protocol_rewards(
        &mut self,
        ncn: Pubkey,
        epoch: u64,
    ) -> TestResult<()> {
        let epoch_state = EpochState::find_program_address(&ncn_program::id(), &ncn, epoch).0;

        let (ncn_config, _, _) = NcnConfig::find_program_address(&ncn_program::id(), &ncn);

        let (ncn_reward_router, _, _) =
            NCNRewardRouter::find_program_address(&ncn_program::id(), &ncn, epoch);

        let ncn_config_account = self.get_ncn_config(ncn).await?;
        let protocol_fee_wallet = ncn_config_account.fee_config.protocol_fee_wallet();

        let (ncn_reward_receiver, _, _) =
            NCNRewardReceiver::find_program_address(&ncn_program::id(), &ncn, epoch);

        let ix = DistributeProtocolRewardsBuilder::new()
            .epoch_state(epoch_state)
            .config(ncn_config)
            .ncn(ncn)
            .ncn_reward_router(ncn_reward_router)
            .ncn_reward_receiver(ncn_reward_receiver)
            .protocol_fee_wallet(*protocol_fee_wallet)
            .system_program(system_program::id())
            .epoch(epoch)
            .instruction();

        let blockhash = self.banks_client.get_latest_blockhash().await?;

        let transaction = Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.payer.pubkey()),
            &[&self.payer],
            blockhash,
        );

        self.process_transaction(&transaction).await
    }

    pub async fn do_distribute_ncn_rewards(&mut self, ncn: Pubkey, epoch: u64) -> TestResult<()> {
        let epoch_state = EpochState::find_program_address(&ncn_program::id(), &ncn, epoch).0;

        let (ncn_config, _, _) = NcnConfig::find_program_address(&ncn_program::id(), &ncn);

        let (ncn_reward_router, _, _) =
            NCNRewardRouter::find_program_address(&ncn_program::id(), &ncn, epoch);

        let ncn_config_account = self.get_ncn_config(ncn).await?;
        let ncn_fee_wallet = ncn_config_account.fee_config.ncn_fee_wallet();

        let (ncn_reward_receiver, _, _) =
            NCNRewardReceiver::find_program_address(&ncn_program::id(), &ncn, epoch);

        let ix = DistributeNCNRewardsBuilder::new()
            .epoch_state(epoch_state)
            .config(ncn_config)
            .ncn(ncn)
            .ncn_reward_router(ncn_reward_router)
            .ncn_reward_receiver(ncn_reward_receiver)
            .ncn_fee_wallet(*ncn_fee_wallet)
            .system_program(system_program::id())
            .epoch(epoch)
            .instruction();

        let blockhash = self.banks_client.get_latest_blockhash().await?;

        let transaction = Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.payer.pubkey()),
            &[&self.payer],
            blockhash,
        );

        self.process_transaction(&transaction).await
    }

    pub async fn do_distribute_operator_vault_reward_route(
        &mut self,
        operator: Pubkey,
        ncn: Pubkey,
        epoch: u64,
    ) -> TestResult<()> {
        let (ncn_config, _, _) = NcnConfig::find_program_address(&ncn_program::id(), &ncn);

        let (ncn_reward_router, _, _) =
            NCNRewardRouter::find_program_address(&ncn_program::id(), &ncn, epoch);
        let (ncn_reward_receiver, _, _) =
            NCNRewardReceiver::find_program_address(&ncn_program::id(), &ncn, epoch);

        let (operator_vault_reward_router, _, _) = OperatorVaultRewardRouter::find_program_address(
            &ncn_program::id(),
            &operator,
            &ncn,
            epoch,
        );
        let (operator_vault_reward_receiver, _, _) =
            OperatorVaultRewardReceiver::find_program_address(
                &ncn_program::id(),
                &operator,
                &ncn,
                epoch,
            );

        self.distribute_operator_vault_reward_route(
            operator,
            ncn,
            ncn_config,
            ncn_reward_router,
            ncn_reward_receiver,
            operator_vault_reward_router,
            operator_vault_reward_receiver,
            epoch,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn distribute_operator_vault_reward_route(
        &mut self,
        operator: Pubkey,
        ncn: Pubkey,
        ncn_config: Pubkey,
        ncn_reward_router: Pubkey,
        ncn_reward_receiver: Pubkey,
        operator_vault_reward_router: Pubkey,
        operator_vault_reward_receiver: Pubkey,
        epoch: u64,
    ) -> TestResult<()> {
        let epoch_state = EpochState::find_program_address(&ncn_program::id(), &ncn, epoch).0;

        let ix = DistributeOperatorVaultRewardRouteBuilder::new()
            .epoch_state(epoch_state)
            .config(ncn_config)
            .ncn(ncn)
            .operator(operator)
            .ncn_reward_router(ncn_reward_router)
            .ncn_reward_receiver(ncn_reward_receiver)
            .operator_vault_reward_receiver(operator_vault_reward_receiver)
            .operator_vault_reward_router(operator_vault_reward_router)
            .system_program(system_program::id())
            .epoch(epoch)
            .instruction();

        let blockhash = self.banks_client.get_latest_blockhash().await?;
        self.process_transaction(&Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.payer.pubkey()),
            &[&self.payer],
            blockhash,
        ))
        .await
    }

    pub async fn do_route_operator_vault_rewards(
        &mut self,
        ncn: Pubkey,
        operator: Pubkey,
        epoch: u64,
    ) -> TestResult<()> {
        let (operator_snapshot, _, _) =
            OperatorSnapshot::find_program_address(&ncn_program::id(), &operator, &ncn, epoch);

        let (operator_vault_reward_router, _, _) = OperatorVaultRewardRouter::find_program_address(
            &ncn_program::id(),
            &operator,
            &ncn,
            epoch,
        );

        let (operator_vault_reward_receiver, _, _) =
            OperatorVaultRewardReceiver::find_program_address(
                &ncn_program::id(),
                &operator,
                &ncn,
                epoch,
            );

        let max_iterations: u16 = OperatorVaultRewardRouter::MAX_ROUTE_NCN_ITERATIONS;
        let mut still_routing = true;

        while still_routing {
            self.route_operator_vault_rewards(
                ncn,
                operator,
                operator_snapshot,
                operator_vault_reward_router,
                operator_vault_reward_receiver,
                max_iterations,
                epoch,
            )
            .await?;

            let ncn_reward_router_account = self
                .get_operator_vault_reward_router(operator, ncn, epoch)
                .await?;

            still_routing = ncn_reward_router_account.still_routing();
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn route_operator_vault_rewards(
        &mut self,
        ncn: Pubkey,
        operator: Pubkey,
        operator_snapshot: Pubkey,
        operator_vault_reward_router: Pubkey,
        operator_vault_reward_receiver: Pubkey,
        max_iterations: u16,
        epoch: u64,
    ) -> TestResult<()> {
        let epoch_state = EpochState::find_program_address(&ncn_program::id(), &ncn, epoch).0;

        let ix = RouteOperatorVaultRewardsBuilder::new()
            .epoch_state(epoch_state)
            .ncn(ncn)
            .operator(operator)
            .operator_snapshot(operator_snapshot)
            .operator_vault_reward_router(operator_vault_reward_router)
            .operator_vault_reward_receiver(operator_vault_reward_receiver)
            .max_iterations(max_iterations)
            .epoch(epoch)
            .instruction();

        let blockhash = self.get_best_latest_blockhash().await?;
        self.process_transaction(&Transaction::new_signed_with_payer(
            &[
                // TODO: should make this instruction much more efficient
                ComputeBudgetInstruction::set_compute_unit_limit(1_400_000),
                ix,
            ],
            Some(&self.payer.pubkey()),
            &[&self.payer],
            blockhash,
        ))
        .await
    }

    pub async fn do_distribute_operator_rewards(
        &mut self,
        operator: Pubkey,
        ncn: Pubkey,
        epoch: u64,
    ) -> TestResult<()> {
        let ncn_config = NcnConfig::find_program_address(&ncn_program::id(), &ncn).0;

        let (operator_vault_reward_router, _, _) = OperatorVaultRewardRouter::find_program_address(
            &ncn_program::id(),
            &operator,
            &ncn,
            epoch,
        );

        let (operator_vault_reward_receiver, _, _) =
            OperatorVaultRewardReceiver::find_program_address(
                &ncn_program::id(),
                &operator,
                &ncn,
                epoch,
            );

        let operator_snapshot =
            OperatorSnapshot::find_program_address(&ncn_program::id(), &operator, &ncn, epoch).0;

        let epoch_state = EpochState::find_program_address(&ncn_program::id(), &ncn, epoch).0;

        let ix = DistributeOperatorRewardsBuilder::new()
            .epoch_state(epoch_state)
            .config(ncn_config)
            .ncn(ncn)
            .operator(operator)
            .operator_snapshot(operator_snapshot)
            .operator_vault_reward_router(operator_vault_reward_router)
            .operator_vault_reward_receiver(operator_vault_reward_receiver)
            .system_program(system_program::id())
            .epoch(epoch)
            .instruction();

        let blockhash = self.banks_client.get_latest_blockhash().await?;
        self.process_transaction(&Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.payer.pubkey()),
            &[&self.payer],
            blockhash,
        ))
        .await
    }

    pub async fn do_distribute_vault_rewards(
        &mut self,
        vault: Pubkey,
        operator: Pubkey,
        ncn: Pubkey,
        epoch: u64,
    ) -> TestResult<()> {
        let ncn_config = NcnConfig::find_program_address(&ncn_program::id(), &ncn).0;

        let (operator_vault_reward_router, _, _) = OperatorVaultRewardRouter::find_program_address(
            &ncn_program::id(),
            &operator,
            &ncn,
            epoch,
        );
        let (operator_vault_reward_receiver, _, _) =
            OperatorVaultRewardReceiver::find_program_address(
                &ncn_program::id(),
                &operator,
                &ncn,
                epoch,
            );

        let operator_snapshot =
            OperatorSnapshot::find_program_address(&ncn_program::id(), &operator, &ncn, epoch).0;

        let epoch_state = EpochState::find_program_address(&ncn_program::id(), &ncn, epoch).0;

        let ix = DistributeVaultRewardsBuilder::new()
            .epoch_state(epoch_state)
            .config(ncn_config)
            .ncn(ncn)
            .operator(operator)
            .vault(vault)
            .operator_snapshot(operator_snapshot)
            .operator_vault_reward_router(operator_vault_reward_router)
            .operator_vault_reward_receiver(operator_vault_reward_receiver)
            .system_program(system_program::id())
            .epoch(epoch)
            .instruction();

        let blockhash = self.banks_client.get_latest_blockhash().await?;
        self.process_transaction(&Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.payer.pubkey()),
            &[&self.payer],
            blockhash,
        ))
        .await
    }
}

/// Asserts that a TestResult contains a specific NCNProgramError.
#[inline(always)]
#[track_caller]
pub fn assert_ncn_program_error<T>(
    test_error: Result<T, TestError>,
    ncn_program_error: NCNProgramError,
    instruction_index: Option<u8>,
) {
    assert!(test_error.is_err());
    assert_eq!(
        test_error.err().unwrap().to_transaction_error().unwrap(),
        TransactionError::InstructionError(
            instruction_index.unwrap_or(0),
            InstructionError::Custom(ncn_program_error as u32)
        )
    );
}
