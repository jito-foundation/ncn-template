#[cfg(test)]
mod tests {
    use jito_restaking_core::{config::Config, ncn_vault_ticket::NcnVaultTicket};
    use ncn_program_core::{ballot_box::WeatherStatus, constants::WEIGHT, error::NCNProgramError};

    use solana_sdk::{msg, signature::Keypair, signer::Signer};

    use crate::fixtures::{
        ncn_program_client::assert_ncn_program_error, test_builder::TestBuilder, TestResult,
    };

    // #[ignore = "20-30 minute test"]
    #[tokio::test]
    async fn simulation_test() -> TestResult<()> {
        // 0.a. Building the test environment
        let mut fixture = TestBuilder::new().await;
        // 0.b. Initialize the configuration for the staking and vault programs
        // you will not have to do that on mainnet, the programs will already be configured
        fixture.initialize_staking_and_vault_programs().await?;

        let mut ncn_program_client = fixture.ncn_program_client();
        let mut vault_program_client = fixture.vault_client();
        let mut restaking_client = fixture.restaking_program_client();

        // 1. Preparing the test variables
        const OPERATOR_COUNT: usize = 13; // Number of operators to create for testing
        let mints = vec![
            (Keypair::new(), WEIGHT),     // TKN1 with base weight
            (Keypair::new(), WEIGHT * 2), // TKN2 with double weight
            (Keypair::new(), WEIGHT * 3), // TKN3 with triple weight
            (Keypair::new(), WEIGHT * 4), // TKN4 with quadruple weight
        ];
        let delegations = [
            1,                  // minimum delegation amount
            10_000_000_000,     // 10 tokens
            100_000_000_000,    // 100 tokens
            1_000_000_000_000,  // 1k tokens
            10_000_000_000_000, // 10k tokens
        ];

        // 2. Initializing all the needed accounts using Jito's Staking and Vault programs
        // this step will initialize the NCN account, and all the operators and vaults accounts,
        // it will also initialize the handshake relationships between all the NCN components

        // 2.a. Initialize the test NCN account using the Restaking program By Jito
        let mut test_ncn = fixture.create_test_ncn().await?;
        let ncn_pubkey = test_ncn.ncn_root.ncn_pubkey;

        // 2.b. Initialize the operators using the Restaking program By Jito, and initiate the
        //   handshake relationship between the NCN <> operators
        // Creates OPERATOR_COUNT operators and associates them with the NCN, setting fee to 100 bps (1%)
        fixture
            .add_operators_to_test_ncn(&mut test_ncn, OPERATOR_COUNT, Some(100))
            .await?;

        // 2.c. Initialize the vaults using the Vault program By Jito
        // and initiate the handshake relationship between the NCN <> vaults, and vaults <> operators
        {
            // Create 3 vaults for TKN1
            fixture
                .add_vaults_to_test_ncn(&mut test_ncn, 3, Some(mints[0].0.insecure_clone()))
                .await?;
            // Create 2 vaults for TKN2
            fixture
                .add_vaults_to_test_ncn(&mut test_ncn, 2, Some(mints[1].0.insecure_clone()))
                .await?;
            // Create 1 vault for TKN3
            fixture
                .add_vaults_to_test_ncn(&mut test_ncn, 1, Some(mints[2].0.insecure_clone()))
                .await?;
            // Create 1 vault for TKN4
            fixture
                .add_vaults_to_test_ncn(&mut test_ncn, 1, Some(mints[3].0.insecure_clone()))
                .await?;
        }

        // 2.d. Vaults delegate stakes to operators
        // Each vault delegates different amounts to different operators based on the delegation amounts array
        {
            for (index, operator_root) in test_ncn
                .operators
                .iter()
                .take(OPERATOR_COUNT - 1) // All operators except the last one
                .enumerate()
            {
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

        // 3. Setting up the NCN-program
        // every thing here will be a call for an instruction to the NCN program that the NCN admin
        // is suppose to deploy to the network.
        {
            // 3.a. Initialize the config for the ncn-program
            ncn_program_client
                .do_initialize_config(test_ncn.ncn_root.ncn_pubkey, &test_ncn.ncn_root.ncn_admin)
                .await?;

            // 3.b Initialize the vault_registry - creates accounts to track vaults
            ncn_program_client
                .do_full_initialize_vault_registry(test_ncn.ncn_root.ncn_pubkey)
                .await?;

            // Fast-forward time to simulate a full epoch passing
            // This is needed for all the relationships to get activated
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

            // 3.c. Register all the ST (Support Token) mints in the ncn program
            // This assigns weights to each mint for voting power calculations
            for (mint, weight) in mints.iter() {
                ncn_program_client
                    .do_admin_register_st_mint(ncn_pubkey, mint.pubkey(), *weight)
                    .await?;
            }

            // 4.d Register all the vaults in the ncn program
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
        // At this point, all the preparations and configurations are done, everything else after
        // this is part of the voting cycle, so it depends on the way you setup your voting system
        // you will have to run the code below
        //
        // in this example, the voting is cyclecle, and per epoch, so the code you will see below
        // will run per epoch to prepare for the voting

        // 4. Prepare the voting environment
        {
            // 4.a. Initialize the epoch state - creates a new state for the current epoch
            fixture.add_epoch_state_for_test_ncn(&test_ncn).await?;
            // 4.b. Initialize the weight table - prepares the table that will track voting weights
            let clock = fixture.clock().await;
            let epoch = clock.epoch;
            ncn_program_client
                .do_full_initialize_weight_table(test_ncn.ncn_root.ncn_pubkey, epoch)
                .await?;

            // 4.c. Take a snapshot of the weights for each ST mint
            // This records the current weights for the voting calculations
            ncn_program_client
                .do_set_epoch_weights(test_ncn.ncn_root.ncn_pubkey, epoch)
                .await?;
            // 4.d. Take the epoch snapshot - records the current state for this epoch
            fixture.add_epoch_snapshot_to_test_ncn(&test_ncn).await?;
            // 4.e. Take a snapshot for each operator - records their current stakes
            fixture
                .add_operator_snapshots_to_test_ncn(&test_ncn)
                .await?;
            // 4.f. Take a snapshot for each vault and its delegation - records delegations
            fixture
                .add_vault_operator_delegation_snapshots_to_test_ncn(&test_ncn)
                .await?;

            // 4.g. Initialize the ballot box - creates the voting container for this epoch
            fixture.add_ballot_box_to_test_ncn(&test_ncn).await?;
        }

        // Define which weather status we expect to win in the vote
        let winning_weather_status = WeatherStatus::Sunny as u8;

        // 5. Cast votes from operators
        {
            let epoch = fixture.clock().await.epoch;

            let zero_delegation_operator = test_ncn.operators.last().unwrap(); // Operator with no delegations
            let first_operator = &test_ncn.operators[0];
            let second_operator = &test_ncn.operators[1];
            let third_operator = &test_ncn.operators[2];

            // Vote from zero_delegation_operator (should fail with an error since operators with zero delegations cannot vote)
            {
                // Verify the operator has no delegations by checking its snapshot
                let operator_snapshot = ncn_program_client
                    .get_operator_snapshot(
                        zero_delegation_operator.operator_pubkey,
                        ncn_pubkey,
                        epoch,
                    )
                    .await?;

                // Log the current stake weight of the zero delegation operator
                let stake_weight = operator_snapshot.stake_weights().stake_weight();
                msg!("Zero-delegation operator stake weight: {}", stake_weight);

                // Confirm it has zero stake weight
                assert_eq!(
                    stake_weight, 0,
                    "Zero-delegation operator should have zero stake weight"
                );

                let weather_status = WeatherStatus::Rainy as u8;

                // We expect this to fail since the operator has zero delegations
                let result = ncn_program_client
                    .do_cast_vote(
                        ncn_pubkey,
                        zero_delegation_operator.operator_pubkey,
                        &zero_delegation_operator.operator_admin,
                        weather_status,
                        epoch,
                    )
                    .await;

                // Verify that voting with zero delegation returns an error
                assert_ncn_program_error(result, NCNProgramError::CannotVoteWithZeroStake);
            }

            // Continue with operators that have delegations
            // First operator votes for Cloudy
            ncn_program_client
                .do_cast_vote(
                    ncn_pubkey,
                    first_operator.operator_pubkey,
                    &first_operator.operator_admin,
                    WeatherStatus::Cloudy as u8,
                    epoch,
                )
                .await?;

            // Second and third operators vote for Sunny (the expected winner)
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

            // 6. Verify voting results
            let ballot_box = ncn_program_client.get_ballot_box(ncn_pubkey, epoch).await?;
            assert!(ballot_box.has_winning_ballot());
            assert!(ballot_box.is_consensus_reached());
            assert_eq!(
                ballot_box.get_winning_ballot().unwrap().weather_status(),
                winning_weather_status
            );
        }

        // 7. Fetch and verify the consensus_result account
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
                "✅ Consensus Result Verified - Weather Status: {}, Vote Weight: {}, Total Weight: {}, Recorder: {}",
                consensus_result.weather_status(),
                consensus_result.vote_weight(),
                consensus_result.total_vote_weight(),
                consensus_result.consensus_recorder()
            );
        }

        // 8. Close epoch accounts but keep consensus result
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

#[cfg(test)]
mod fuzz_tests {
    use crate::fixtures::{test_builder::TestBuilder, TestResult};
    use jito_restaking_core::{config::Config, ncn_vault_ticket::NcnVaultTicket};
    use ncn_program_core::{
        ballot_box::Ballot,
        constants::{WEIGHT, WEIGHT_PRECISION},
    };
    use solana_sdk::{
        native_token::sol_to_lamports, pubkey::Pubkey, signature::Keypair, signer::Signer,
    };

    // Struct to configure mint token parameters for simulation
    struct MintConfig {
        keypair: Keypair,
        weight: u128,       // Weight for voting power calculation
        vault_count: usize, // Number of vaults to create for this mint
    }

    // Overall simulation configuration
    struct SimConfig {
        operator_count: usize,  // Number of operators to create
        mints: Vec<MintConfig>, // Token mint configurations
        delegations: Vec<u64>,  // Array of delegation amounts for vaults
        operator_fee_bps: u16,  // Operator fee in basis points (100 = 1%)
    }

    // Main simulation function that runs a full voting cycle with the given configuration
    async fn run_simulation(config: SimConfig) -> TestResult<()> {
        // Create test environment
        let mut fixture = TestBuilder::new().await;
        fixture.initialize_staking_and_vault_programs().await?;

        let mut ncn_program_client = fixture.ncn_program_client();
        let mut vault_program_client = fixture.vault_client();
        let mut restaking_client = fixture.restaking_program_client();

        // Validate configuration
        let total_vaults = config.mints.iter().map(|m| m.vault_count).sum::<usize>();
        assert_eq!(config.delegations.len(), total_vaults);

        // Setup Network Coordination Node (NCN)
        let mut test_ncn = fixture.create_test_ncn().await?;
        let ncn = test_ncn.ncn_root.ncn_pubkey;

        // Initialize the NCN Program program for this NCN
        ncn_program_client
            .setup_ncn_program(&test_ncn.ncn_root)
            .await?;

        // Add operators and vaults based on configuration
        {
            // Create operators with specified fee
            fixture
                .add_operators_to_test_ncn(
                    &mut test_ncn,
                    config.operator_count,
                    Some(config.operator_fee_bps),
                )
                .await?;

            // Create vaults for each mint
            for mint_config in config.mints.iter() {
                fixture
                    .add_vaults_to_test_ncn(
                        &mut test_ncn,
                        mint_config.vault_count,
                        Some(mint_config.keypair.insecure_clone()),
                    )
                    .await?;
            }
        }

        // Set up delegation from vaults to operators
        {
            // Create a seed for pseudorandom operator selection
            let seed = Pubkey::new_unique()
                .to_bytes()
                .iter()
                .enumerate()
                .fold(0u64, |acc, (i, &byte)| {
                    acc.wrapping_add((byte as u64) << (i % 8 * 8))
                });

            // For each vault, distribute its delegation among operators
            for (vault_index, vault_root) in test_ncn.vaults.iter().enumerate() {
                let total_vault_delegation = config.delegations[vault_index];

                // Create a shuffled list of operators for randomized distribution
                let mut operators: Vec<_> = test_ncn.operators.iter().collect();
                let shuffle_index = seed.wrapping_add(vault_index as u64);

                // Fisher-Yates shuffle to randomize operator order
                for i in (1..operators.len()).rev() {
                    let j = (shuffle_index.wrapping_mul(i as u64) % (i as u64 + 1)) as usize;
                    operators.swap(i, j);
                }

                // Skip the first operator (effectively excluding them from delegation)
                let selected_operators = operators.iter().skip(1).take(config.operator_count - 2);
                let operator_count = config.operator_count - 2; // Reduced by two to account for exclusions

                // Calculate delegation per operator and distribute
                let delegation_per_operator = total_vault_delegation / operator_count as u64;

                if delegation_per_operator > 0 {
                    for operator_root in selected_operators {
                        vault_program_client
                            .do_add_delegation(
                                vault_root,
                                &operator_root.operator_pubkey,
                                delegation_per_operator,
                            )
                            .await
                            .unwrap();
                    }
                }
            }
        }

        // Register tokens and vaults with the NCN Program
        {
            // Fast-forward time to ensure all relationships are active
            let restaking_config_address =
                Config::find_program_address(&jito_restaking_program::id()).0;
            let restaking_config = restaking_client
                .get_config(&restaking_config_address)
                .await?;
            let epoch_length = restaking_config.epoch_length();

            fixture.warp_slot_incremental(epoch_length * 2).await?;

            // Register each mint token with its weight
            for mint_config in config.mints.iter() {
                ncn_program_client
                    .do_admin_register_st_mint(
                        ncn,
                        mint_config.keypair.pubkey(),
                        mint_config.weight,
                    )
                    .await?;
            }

            // Register each vault with the NCN Program
            for vault in test_ncn.vaults.iter() {
                let vault = vault.vault_pubkey;
                let (ncn_vault_ticket, _, _) = NcnVaultTicket::find_program_address(
                    &jito_restaking_program::id(),
                    &ncn,
                    &vault,
                );

                ncn_program_client
                    .do_register_vault(ncn, vault, ncn_vault_ticket)
                    .await?;
            }
        }

        // Set up the voting environment for the current epoch
        fixture.add_epoch_state_for_test_ncn(&test_ncn).await?;
        fixture.add_weights_for_test_ncn(&test_ncn).await?;

        // Verify weight setup is complete
        {
            let epoch = fixture.clock().await.epoch;
            let epoch_state = ncn_program_client.get_epoch_state(ncn, epoch).await?;
            assert!(epoch_state.set_weight_progress().is_complete())
        }

        // Take snapshots of current state for voting
        fixture.add_epoch_snapshot_to_test_ncn(&test_ncn).await?;
        fixture
            .add_operator_snapshots_to_test_ncn(&test_ncn)
            .await?;
        fixture
            .add_vault_operator_delegation_snapshots_to_test_ncn(&test_ncn)
            .await?;
        fixture.add_ballot_box_to_test_ncn(&test_ncn).await?;

        // Cast votes from all operators for the same weather status
        {
            let epoch = fixture.clock().await.epoch;
            // Generate a random weather status for this test
            let weather_status = Ballot::generate_ballot_weather_status();

            // All operators vote for the same status to ensure consensus
            for operator_root in test_ncn.operators.iter() {
                let operator = operator_root.operator_pubkey;
                let _ = ncn_program_client
                    .do_cast_vote(
                        ncn,
                        operator,
                        &operator_root.operator_admin,
                        weather_status,
                        epoch,
                    )
                    .await;
            }

            // Verify consensus is reached with expected result
            let ballot_box = ncn_program_client.get_ballot_box(ncn, epoch).await?;
            assert!(ballot_box.has_winning_ballot());
            assert!(ballot_box.is_consensus_reached());
            assert_eq!(
                ballot_box.get_winning_ballot().unwrap().weather_status(),
                weather_status
            );
        }

        // Clean up epoch accounts
        fixture.close_epoch_accounts_for_test_ncn(&test_ncn).await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_basic_simulation() -> TestResult<()> {
        // Basic configuration with multiple mints and delegation amounts
        let config = SimConfig {
            operator_count: 13,
            mints: vec![
                MintConfig {
                    keypair: Keypair::new(),
                    weight: WEIGHT,
                    vault_count: 3,
                },
                MintConfig {
                    keypair: Keypair::new(),
                    weight: WEIGHT,
                    vault_count: 2,
                },
                MintConfig {
                    keypair: Keypair::new(),
                    weight: WEIGHT,
                    vault_count: 1,
                },
                MintConfig {
                    keypair: Keypair::new(),
                    weight: WEIGHT_PRECISION, // Minimum weight precision
                    vault_count: 1,
                },
            ],
            delegations: vec![
                // 7 delegation amounts for 7 total vaults
                1,                           // Minimum delegation amount
                sol_to_lamports(1000.0),     // 1,000 SOL
                sol_to_lamports(10000.0),    // 10,000 SOL
                sol_to_lamports(100000.0),   // 100,000 SOL
                sol_to_lamports(1000000.0),  // 1,000,000 SOL
                sol_to_lamports(10000000.0), // 10,000,000 SOL
                255,                         // Arbitrary small amount
            ],
            operator_fee_bps: 100, // 1% operator fee
        };

        run_simulation(config).await
    }

    #[tokio::test]
    async fn test_high_operator_count_simulation() -> TestResult<()> {
        // Test with a large number of operators to verify scalability
        let config = SimConfig {
            operator_count: 50, // High number of operators
            mints: vec![MintConfig {
                keypair: Keypair::new(),
                weight: WEIGHT,
                vault_count: 2,
            }],
            delegations: vec![sol_to_lamports(1000.0), sol_to_lamports(1000.0)],
            operator_fee_bps: 100,
        };

        run_simulation(config).await
    }

    #[tokio::test]
    async fn test_fuzz_simulation() -> TestResult<()> {
        // Create multiple test configurations with different parameters
        let test_configs = vec![
            // Test 1: Mid-size operator set with varied delegation amounts
            SimConfig {
                operator_count: 15,
                mints: vec![
                    MintConfig {
                        keypair: Keypair::new(),
                        weight: WEIGHT,
                        vault_count: 2,
                    },
                    MintConfig {
                        keypair: Keypair::new(),
                        weight: WEIGHT,
                        vault_count: 1,
                    },
                ],
                delegations: vec![
                    sol_to_lamports(500.0),   // Small delegation
                    sol_to_lamports(5000.0),  // Medium delegation
                    sol_to_lamports(50000.0), // Large delegation
                ],
                operator_fee_bps: 90, // 0.9% fee
            },
            // Test 2: Extreme delegation amounts
            SimConfig {
                operator_count: 20,
                mints: vec![MintConfig {
                    keypair: Keypair::new(),
                    weight: 2 * WEIGHT_PRECISION, // Double precision weight
                    vault_count: 3,
                }],
                delegations: vec![
                    1,                            // Minimum possible delegation
                    sol_to_lamports(1.0),         // Very small delegation
                    sol_to_lamports(1_000_000.0), // Extremely large delegation
                ],
                operator_fee_bps: 150, // 1.5% fee
            },
            // Test 3: Mixed token weights and varied delegation amounts
            SimConfig {
                operator_count: 30,
                mints: vec![
                    MintConfig {
                        keypair: Keypair::new(),
                        weight: WEIGHT, // Standard weight
                        vault_count: 1,
                    },
                    MintConfig {
                        keypair: Keypair::new(),
                        weight: WEIGHT * 2, // Double weight
                        vault_count: 1,
                    },
                    MintConfig {
                        keypair: Keypair::new(),
                        weight: WEIGHT_PRECISION / 2, // Half precision weight
                        vault_count: 1,
                    },
                ],
                delegations: vec![
                    sol_to_lamports(100.0),   // Small delegation
                    sol_to_lamports(1000.0),  // Medium delegation
                    sol_to_lamports(10000.0), // Large delegation
                ],
                operator_fee_bps: 80, // 0.8% fee
            },
        ];

        // Run all configurations sequentially
        for (i, config) in test_configs.into_iter().enumerate() {
            println!("Running fuzz test configuration {}", i + 1);
            run_simulation(config).await?;
        }

        Ok(())
    }
}
