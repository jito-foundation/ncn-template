#[cfg(test)]
mod tests {
    use jito_restaking_core::{config::Config, ncn_vault_ticket::NcnVaultTicket};
    use ncn_program_core::{ballot_box::WeatherStatus, constants::WEIGHT};

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
        // Simulate rewards flowing through the system after consensus
        {
            const REWARD_AMOUNT: u64 = 1_000_000;
            // Setup reward routers for NCN and operators
            fixture.add_routers_for_test_ncn(&test_ncn).await?;
            // Route rewards into the NCN reward system
            fixture
                .route_in_ncn_rewards_for_test_ncn(&test_ncn, REWARD_AMOUNT)
                .await?;
            // Route rewards to operators and their delegated vaults
            fixture
                .route_in_operator_vault_rewards_for_test_ncn(&test_ncn)
                .await?;
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
