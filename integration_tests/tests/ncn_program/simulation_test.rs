#[cfg(test)]
mod tests {
    use jito_restaking_core::{config::Config, ncn_vault_ticket::NcnVaultTicket};
    use ncn_program_core::{
        ballot_box::WeatherStatus, constants::WEIGHT, ncn_reward_router::NCNRewardReceiver,
    };

    use solana_sdk::{msg, signature::Keypair, signer::Signer};

    use crate::fixtures::{test_builder::TestBuilder, TestResult};

    // This test runs a complete end-to-end NCN (Network of Consensus Nodes) consensus workflow
    #[tokio::test]
    async fn simulation_test() -> TestResult<()> {
        // 1. Setup test environment
        // 1.a. Building the test environment
        let mut fixture = TestBuilder::new().await;
        // 1.b. Initialize the configuration for the restaking and vault programs
        // Note: On mainnet, these programs would already be configured
        fixture.initialize_restaking_and_vault_programs().await?;

        let mut ncn_program_client = fixture.ncn_program_client();
        let mut vault_program_client = fixture.vault_client();
        let mut restaking_client = fixture.restaking_program_client();

        // 2. Define test parameters
        const OPERATOR_COUNT: usize = 13; // Number of operators to create for testing
        let mints = vec![
            (Keypair::new(), WEIGHT),     // Alice with base weight
            (Keypair::new(), WEIGHT * 2), // Bob with double weight
            (Keypair::new(), WEIGHT * 3), // Charlie with triple weight
            (Keypair::new(), WEIGHT * 4), // Dave with quadruple weight
        ];
        let delegations = [
            1,                  // minimum delegation amount
            10_000_000_000,     // 10 tokens
            100_000_000_000,    // 100 tokens
            1_000_000_000_000,  // 1k tokens
            10_000_000_000_000, // 10k tokens
        ];

        // 3. Initialize system accounts and establish relationships
        // 3.a. Initialize the NCN account using the Jito Restaking program
        let mut test_ncn = fixture.create_test_ncn().await?;
        let ncn_pubkey = test_ncn.ncn_root.ncn_pubkey;

        // 3.b. Initialize operators and establish NCN <> operator relationships
        {
            for _ in 0..OPERATOR_COUNT {
                // Set operator fee to 100 basis points (1%)
                let operator_fees_bps: Option<u16> = Some(100);

                // Initialize a new operator account with the specified fee
                let operator_root = restaking_client
                    .do_initialize_operator(operator_fees_bps)
                    .await?;

                // Establish bidirectional handshake between NCN and operator:
                // 1. Initialize the NCN's state tracking for this operator
                restaking_client
                    .do_initialize_ncn_operator_state(
                        &test_ncn.ncn_root,
                        &operator_root.operator_pubkey,
                    )
                    .await?;

                // 2. Advance slot to satisfy timing requirements
                fixture.warp_slot_incremental(1).await.unwrap();

                // 3. NCN warms up to operator - creates NCN's half of the handshake
                restaking_client
                    .do_ncn_warmup_operator(&test_ncn.ncn_root, &operator_root.operator_pubkey)
                    .await?;

                // 4. Operator warms up to NCN - completes operator's half of the handshake
                restaking_client
                    .do_operator_warmup_ncn(&operator_root, &test_ncn.ncn_root.ncn_pubkey)
                    .await?;

                // Add the initialized operator to our test NCN's operator list
                test_ncn.operators.push(operator_root);
            }
        }

        // 3.c. Initialize vaults and establish NCN <> vaults and vault <> operator relationships
        {
            // Create 3 vaults for Alice
            fixture
                .add_vaults_to_test_ncn(&mut test_ncn, 3, Some(mints[0].0.insecure_clone()))
                .await?;
            // Create 2 vaults for Bob
            fixture
                .add_vaults_to_test_ncn(&mut test_ncn, 2, Some(mints[1].0.insecure_clone()))
                .await?;
            // Create 1 vault for Charlie
            fixture
                .add_vaults_to_test_ncn(&mut test_ncn, 1, Some(mints[2].0.insecure_clone()))
                .await?;
            // Create 1 vault for Dave
            fixture
                .add_vaults_to_test_ncn(&mut test_ncn, 1, Some(mints[3].0.insecure_clone()))
                .await?;
        }

        // 3.d. Vaults delegate stakes to operators
        // Each vault delegates different amounts to different operators based on the delegation amounts array
        {
            for (index, operator_root) in test_ncn.operators.iter().enumerate() {
                for vault_root in test_ncn.vaults.iter() {
                    // Cycle through delegation amounts based on operator index
                    let delegation_amount = delegations[index % delegations.len()];

                    if delegation_amount > 0 {
                        vault_program_client
                            .do_add_delegation(
                                vault_root,
                                &operator_root.operator_pubkey,
                                delegation_amount,
                            )
                            .await
                            .unwrap();
                    }
                }
            }
        }

        // 3.e. Fast-forward time to simulate a full epoch passing
        // This is needed for all the relationships to finish warming up
        {
            let restaking_config_address =
                Config::find_program_address(&jito_restaking_program::id()).0;
            let restaking_config = restaking_client
                .get_config(&restaking_config_address)
                .await?;
            let epoch_length = restaking_config.epoch_length();
            fixture
                .warp_slot_incremental(epoch_length * 2)
                .await
                .unwrap();
        }

        // 4. Setting up the NCN-program
        // The following instructions would be executed by the NCN admin in a production environment
        {
            // 4.a. Initialize the config for the NCN program
            ncn_program_client
                .do_initialize_config(test_ncn.ncn_root.ncn_pubkey, &test_ncn.ncn_root.ncn_admin)
                .await?;

            // 4.b Initialize the vault_registry - creates accounts to track vaults
            ncn_program_client
                .do_full_initialize_vault_registry(test_ncn.ncn_root.ncn_pubkey)
                .await?;

            // 4.c. Register all the Supported Token (ST) mints in the NCN program
            // This assigns weights to each mint for voting power calculations
            for (mint, weight) in mints.iter() {
                ncn_program_client
                    .do_admin_register_st_mint(ncn_pubkey, mint.pubkey(), *weight)
                    .await?;
            }

            // 4.d Register all the vaults in the NCN program
            // This is permissionless because the admin already approved it by initiating
            // the handshake before
            for vault in test_ncn.vaults.iter() {
                let vault = vault.vault_pubkey;
                let (ncn_vault_ticket, _, _) = NcnVaultTicket::find_program_address(
                    &jito_restaking_program::id(),
                    &ncn_pubkey,
                    &vault,
                );

                ncn_program_client
                    .do_register_vault(ncn_pubkey, vault, ncn_vault_ticket)
                    .await?;
            }
        }

        // 5. Prepare the epoch consensus cycle
        // In a real system, these steps would run each epoch to prepare for voting on weather status
        {
            // 5.a. Initialize the epoch state - creates a new state for the current epoch
            fixture.add_epoch_state_for_test_ncn(&test_ncn).await?;

            // 5.b. Initialize the weight table - prepares the table that will track voting weights
            let clock = fixture.clock().await;
            let epoch = clock.epoch;
            ncn_program_client
                .do_full_initialize_weight_table(test_ncn.ncn_root.ncn_pubkey, epoch)
                .await?;

            // 5.c. Take a snapshot of the weights for each ST mint
            // This records the current weights for the voting calculations
            ncn_program_client
                .do_set_epoch_weights(test_ncn.ncn_root.ncn_pubkey, epoch)
                .await?;

            // 5.d. Take the epoch snapshot - records the current state for this epoch
            fixture.add_epoch_snapshot_to_test_ncn(&test_ncn).await?;

            // 5.e. Take a snapshot for each operator - records their current stakes
            fixture
                .add_operator_snapshots_to_test_ncn(&test_ncn)
                .await?;

            // 5.f. Take a snapshot for each vault and its delegation - records delegations
            fixture
                .add_vault_operator_delegation_snapshots_to_test_ncn(&test_ncn)
                .await?;

            // 5.g. Initialize the ballot box - creates the voting container for this epoch
            fixture.add_ballot_box_to_test_ncn(&test_ncn).await?;
        }

        // Define which weather status we expect to win in the vote
        // In this example, operators will vote on a simulated weather status
        let winning_weather_status = WeatherStatus::Sunny as u8;

        // 6. Cast votes from operators
        {
            let epoch = fixture.clock().await.epoch;

            let first_operator = &test_ncn.operators[0];
            let second_operator = &test_ncn.operators[1];
            let third_operator = &test_ncn.operators[2];

            // First operator votes for Cloudy (minority vote)
            ncn_program_client
                .do_cast_vote(
                    ncn_pubkey,
                    first_operator.operator_pubkey,
                    &first_operator.operator_admin,
                    WeatherStatus::Cloudy as u8,
                    epoch,
                )
                .await?;

            // Second and third operators vote for Sunny (expected winner)
            ncn_program_client
                .do_cast_vote(
                    ncn_pubkey,
                    second_operator.operator_pubkey,
                    &second_operator.operator_admin,
                    winning_weather_status,
                    epoch,
                )
                .await?;
            ncn_program_client
                .do_cast_vote(
                    ncn_pubkey,
                    third_operator.operator_pubkey,
                    &third_operator.operator_admin,
                    winning_weather_status,
                    epoch,
                )
                .await?;

            // All remaining operators also vote for Sunny to form a majority
            for operator_root in test_ncn.operators.iter().take(OPERATOR_COUNT - 1).skip(3) {
                let operator = operator_root.operator_pubkey;

                ncn_program_client
                    .do_cast_vote(
                        ncn_pubkey,
                        operator,
                        &operator_root.operator_admin,
                        winning_weather_status,
                        epoch,
                    )
                    .await?;
            }

            // 7. Verify voting results
            let ballot_box = ncn_program_client.get_ballot_box(ncn_pubkey, epoch).await?;
            assert!(ballot_box.has_winning_ballot());
            assert!(ballot_box.is_consensus_reached());
            assert_eq!(
                ballot_box.get_winning_ballot().unwrap().weather_status(),
                winning_weather_status
            );
        }

        // 8. Reward Distribution
        // This section simulates the flow of rewards through the system after a successful
        // consensus vote. It covers setting up reward-related accounts, funding the reward pool,
        // and distributing rewards to various stakeholders: the protocol, the NCN, operators, and vaults.
        {
            const REWARD_AMOUNT: u64 = 1_000_000;

            // 8.1. Setup reward routers for NCN and operators
            // Before rewards can be distributed, specialized accounts called "reward routers"
            // must be initialized for the current epoch. These accounts manage the splitting
            // and distribution of funds.
            {
                let mut ncn_program_client = fixture.ncn_program_client();
                let ncn = test_ncn.ncn_root.ncn_pubkey;
                let clock = fixture.clock().await;
                let epoch = clock.epoch;

                // Initialize the main reward router for the NCN. This account acts as the
                // primary hub for incoming rewards for the epoch.
                ncn_program_client
                    .do_full_initialize_ncn_reward_router(ncn, epoch)
                    .await?;

                // For each operator, initialize an operator-specific reward router. This router
                // will handle the distribution of rewards between the operator (as a fee)
                // and the vaults that have delegated to it.
                for operator_root in test_ncn.operators.iter() {
                    let operator = operator_root.operator_pubkey;

                    ncn_program_client
                        .do_initialize_operator_vault_reward_router(ncn, operator, epoch)
                        .await?;
                }
            }

            // 8.2. Route rewards into the NCN reward system
            // This block handles the initial injection of rewards and the first level of distribution.
            {
                let mut ncn_program_client = fixture.ncn_program_client();
                let ncn = test_ncn.ncn_root.ncn_pubkey;
                let epoch = fixture.clock().await.epoch;

                // Advance the clock to ensure we are in a valid time window for reward distribution.
                let valid_slots_after_consensus = {
                    let config = ncn_program_client.get_ncn_config(ncn).await?;
                    config.valid_slots_after_consensus()
                };

                fixture
                    .warp_slot_incremental(valid_slots_after_consensus + 1)
                    .await?;

                // The NCNRewardReceiver is a Program Derived Address (PDA) that serves as the
                // entry point for all rewards for a given NCN and epoch.
                let ncn_reward_receiver =
                    NCNRewardReceiver::find_program_address(&ncn_program::id(), &ncn, epoch).0;

                fn lamports_to_sol(lamports: u64) -> f64 {
                    lamports as f64 / 1_000_000_000.0
                }

                let sol_rewards = lamports_to_sol(REWARD_AMOUNT);

                // Simulate rewards being sent to the system by airdropping SOL to the receiver account.
                ncn_program_client
                    .airdrop(&ncn_reward_receiver, sol_rewards)
                    .await?;

                // Trigger the initial routing of rewards from the main receiver. This instruction
                // splits the total rewards into sub-pools for the protocol, the NCN, and the
                // collective of operators/vaults. Calling it twice demonstrates idempotency.
                ncn_program_client.do_route_ncn_rewards(ncn, epoch).await?;
                // Should be able to route twice
                ncn_program_client.do_route_ncn_rewards(ncn, epoch).await?;

                // Fetch the state of the NCN reward router to verify the distribution.
                let ncn_reward_router =
                    ncn_program_client.get_ncn_reward_router(ncn, epoch).await?;

                // 8.2.1. Protocol Rewards Distribution
                // Distribute the portion of rewards allocated to the protocol.
                {
                    let rewards = ncn_reward_router.protocol_rewards();

                    if rewards > 0 {
                        let mut ncn_program_client = fixture.ncn_program_client();
                        let config = ncn_program_client.get_ncn_config(ncn).await?;
                        let protocol_fee_wallet = config.fee_config.protocol_fee_wallet();

                        let balance_before = {
                            let account = fixture.get_account(protocol_fee_wallet).await?;
                            account.unwrap().lamports
                        };

                        println!("Distributing {} of Protocol Rewards", rewards);
                        ncn_program_client
                            .do_distribute_protocol_rewards(ncn, epoch)
                            .await?;

                        let balance_after = {
                            let account = fixture.get_account(protocol_fee_wallet).await?;
                            account.unwrap().lamports
                        };

                        // Verify that the protocol's fee wallet balance increased by the exact reward amount.
                        assert_eq!(
                            balance_after,
                            balance_before + rewards,
                            "Protocol fee wallet balance should increase by the rewards amount"
                        );
                    }
                }

                // 8.2.2. NCN Rewards Distribution
                // Distribute the portion of rewards allocated to the NCN itself.
                {
                    let rewards = ncn_reward_router.ncn_rewards();

                    if rewards > 0 {
                        let mut ncn_program_client = fixture.ncn_program_client();
                        let config = ncn_program_client.get_ncn_config(ncn).await?;
                        let ncn_fee_wallet = config.fee_config.ncn_fee_wallet();

                        let balance_before = {
                            let account = fixture.get_account(ncn_fee_wallet).await?;
                            account.unwrap().lamports
                        };

                        println!("Distributing {} of NCN Rewards", rewards);
                        ncn_program_client
                            .do_distribute_ncn_rewards(ncn, epoch)
                            .await?;

                        let balance_after = {
                            let account = fixture.get_account(ncn_fee_wallet).await?;
                            account.unwrap().lamports
                        };

                        // Verify that the NCN's fee wallet balance increased by the exact reward amount.
                        assert_eq!(
                            balance_after,
                            balance_before + rewards,
                            "NCN fee wallet balance should increase by the rewards amount"
                        );
                    }
                }

                // 8.2.3. Operator Vault Rewards Distribution
                // Distribute rewards from the main NCN router to each operator's individual
                // reward router. This is a transfer between program-controlled accounts.
                {
                    for operator_root in test_ncn.operators.iter() {
                        let operator = operator_root.operator_pubkey;

                        let operator_route =
                            ncn_reward_router.operator_vault_reward_route(&operator);

                        let rewards = operator_route.rewards().unwrap_or(0);

                        if rewards == 0 {
                            continue;
                        }

                        println!("Distribute Ncn Reward {}", rewards);
                        // This instruction moves funds from the NCN reward router to the operator's
                        // vault reward router for further distribution.
                        ncn_program_client
                            .do_distribute_operator_vault_reward_route(operator, ncn, epoch)
                            .await?;
                    }
                }
            }

            // 8.3. Route rewards to operators and their delegated vaults
            // This block handles the second level of distribution: from each operator's
            // reward router to the operator (fees) and the vaults that delegated to them.
            {
                let mut ncn_program_client = fixture.ncn_program_client();
                let ncn = test_ncn.ncn_root.ncn_pubkey;
                let epoch = fixture.clock().await.epoch;

                for operator_root in test_ncn.operators.iter() {
                    let operator = operator_root.operator_pubkey;

                    // This instruction processes the rewards within an operator's reward router,
                    // splitting the funds based on operator fees and vault delegations.
                    // Calling it twice demonstrates idempotency.
                    ncn_program_client
                        .do_route_operator_vault_rewards(ncn, operator, epoch)
                        .await?;
                    // Should be able to route twice
                    ncn_program_client
                        .do_route_operator_vault_rewards(ncn, operator, epoch)
                        .await?;

                    // Fetch the state of the operator's reward router to get the amounts.
                    let operator_vault_reward_router = ncn_program_client
                        .get_operator_vault_reward_router(operator, ncn, epoch)
                        .await?;

                    // 8.3.1. Distribute Operator Rewards
                    // Pay out the operator's share of the rewards (their fees).
                    let operator_rewards = operator_vault_reward_router.operator_rewards();
                    if operator_rewards > 0 {
                        ncn_program_client
                            .do_distribute_operator_rewards(operator, ncn, epoch)
                            .await?;
                    }

                    // 8.3.2. Distribute Vault Rewards
                    // Pay out the rewards to each vault that delegated to this operator.
                    for vault_root in test_ncn.vaults.iter() {
                        let vault = vault_root.vault_pubkey;

                        let vault_reward_route =
                            operator_vault_reward_router.vault_reward_route(&vault);

                        if let Ok(vault_reward_route) = vault_reward_route {
                            let vault_rewards = vault_reward_route.rewards();

                            if vault_rewards > 0 {
                                // Transfer the final reward amount to the vault.
                                ncn_program_client
                                    .do_distribute_vault_rewards(vault, operator, ncn, epoch)
                                    .await?;
                            }
                        }
                    }
                }
            }
        }

        // 9. Fetch and verify the consensus result account
        {
            let epoch = fixture.clock().await.epoch;
            let consensus_result = ncn_program_client
                .get_consensus_result(ncn_pubkey, epoch)
                .await?;

            // Verify consensus_result account exists and has correct values
            assert!(consensus_result.is_consensus_reached());
            assert_eq!(consensus_result.epoch(), epoch);
            assert_eq!(consensus_result.weather_status(), winning_weather_status);

            // Get ballot box to compare values
            let ballot_box = ncn_program_client.get_ballot_box(ncn_pubkey, epoch).await?;
            msg!("Ballot Box: {}", ballot_box);
            msg!("consensus_result: {}", consensus_result);
            let winning_ballot_tally = ballot_box.get_winning_ballot_tally().unwrap();

            // Verify vote weights match between ballot box and consensus result
            assert_eq!(
                consensus_result.vote_weight(),
                winning_ballot_tally.stake_weights().stake_weight() as u64
            );

            println!(
                "✅ Consensus Result Verified - Weather Status: {}, Vote Weight: {}, Total Weight: {}",
                consensus_result.weather_status(),
                consensus_result.vote_weight(),
                consensus_result.total_vote_weight(),
            );
        }

        // 10. Close epoch accounts but keep consensus result
        // This simulates cleanup after epoch completion while preserving the final result
        let epoch_before_closing_account = fixture.clock().await.epoch;
        fixture.close_epoch_accounts_for_test_ncn(&test_ncn).await?;

        // Verify that consensus_result account is not closed (it should persist)
        {
            let consensus_result = ncn_program_client
                .get_consensus_result(ncn_pubkey, epoch_before_closing_account)
                .await?;

            // Verify consensus_result account exists and has correct values
            assert!(consensus_result.is_consensus_reached());
            assert_eq!(consensus_result.epoch(), epoch_before_closing_account);
        }

        Ok(())
    }
}
