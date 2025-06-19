#[cfg(test)]
mod fuzz_tests {
    use crate::fixtures::{test_builder::TestBuilder, TestResult};
    use jito_restaking_core::{config::Config, ncn_vault_ticket::NcnVaultTicket};
    use ncn_program_core::{ballot_box::WeatherStatus, constants::WEIGHT};
    use solana_sdk::{
        msg, native_token::sol_to_lamports, pubkey::Pubkey, signature::Keypair, signer::Signer,
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

    // Main simulation function that runs a full consensus cycle with the given configuration
    async fn run_simulation(config: SimConfig) -> TestResult<()> {
        // Create test environment
        let mut fixture = TestBuilder::new().await;
        fixture.initialize_restaking_and_vault_programs().await?;

        let mut ncn_program_client = fixture.ncn_program_client();
        let mut vault_program_client = fixture.vault_client();
        let mut restaking_client = fixture.restaking_program_client();

        // Validate configuration
        let total_vaults = config.mints.iter().map(|m| m.vault_count).sum::<usize>();
        assert_eq!(config.delegations.len(), total_vaults);

        // 2. Initializing all the needed accounts using Jito's Staking and Vault programs
        // this step will initialize the NCN account, and all the operators and vaults accounts,
        // it will also initialize the handshake relationships between all the NCN components

        // 2.a. Initialize the NCN account using the Jito Restaking program
        let mut test_ncn = fixture.create_test_ncn().await?;
        let ncn_pubkey = test_ncn.ncn_root.ncn_pubkey;

        // 2.b. Initialize the operators using the Jito Restaking program, and initiate the
        //   handshake relationship between the NCN <> operators
        {
            for _ in 0..config.operator_count {
                // Set operator fee to the configured value
                let operator_fees_bps: Option<u16> = Some(config.operator_fee_bps);

                // Initialize a new operator account with the specified fee
                let operator_root = restaking_client
                    .do_initialize_operator(operator_fees_bps)
                    .await?;

                // Establish bidirectional handshake between NCN and operator:
                // 1. Initialize the NCN's state tracking (the NCN operator ticket) for this operator
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

        // 2.c. Initialize the vaults using the Vault program By Jito
        // and initiate the handshake relationship between the NCN <> vaults, and vaults <> operators
        {
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

        // 2.d. Vaults delegate stakes to operators
        // Each vault delegates different amounts to different operators based on the delegation amounts array
        {
            for (_, operator_root) in test_ncn.operators.iter().enumerate() {
                for (vault_index, vault_root) in test_ncn.vaults.iter().enumerate() {
                    // Use the delegation amount for this specific vault
                    let delegation_amount = config.delegations[vault_index];

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

        {
            // 2.e Fast-forward time to simulate a full epoch passing
            // This is needed for all the relationships to finish warming up
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

            // 3.c. Register all the ST (Support Token) mints in the ncn program
            // This assigns weights to each mint for voting power calculations
            for mint_config in config.mints.iter() {
                ncn_program_client
                    .do_admin_register_st_mint(
                        ncn_pubkey,
                        mint_config.keypair.pubkey(),
                        mint_config.weight,
                    )
                    .await?;
            }

            // 3.d Register all the vaults in the ncn program
            // note that this is permissionless because the admin already approved it by initiating
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
        // At this point, all the preparations and configurations are done, everything else after
        // this is part of the consensus cycle, so it depends on the way you setup your voting system
        // you will have to run the code below
        //
        // in this example, the voting is cyclical, and per epoch, so the code you will see below
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

            // All operators vote for the same status to ensure consensus
            for operator_root in test_ncn.operators.iter() {
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

        // 7. Reward Distribution
        {
            const REWARD_AMOUNT: u64 = 1_000_000;
            fixture.add_routers_for_test_ncn(&test_ncn).await?;
            fixture
                .route_in_ncn_rewards_for_test_ncn(&test_ncn, REWARD_AMOUNT)
                .await?;

            fixture
                .route_in_operator_vault_rewards_for_test_ncn(&test_ncn)
                .await?;
        }

        // 8. Fetch and verify the consensus_result account
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

        // 9. Close epoch accounts but keep consensus result
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
                    weight: WEIGHT,
                    vault_count: 1,
                },
            ],
            delegations: vec![
                // 7 delegation amounts for 7 total vaults
                sol_to_lamports(1.0),   // 1 SOL
                sol_to_lamports(10.0),  // 10 SOL
                sol_to_lamports(100.0), // 100 SOL
                sol_to_lamports(10.0),  // 10 SOL
                sol_to_lamports(1.0),   // 1 SOL
                255,                    // Arbitrary small amount
                1,                      // Minimum delegation amount
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
                    sol_to_lamports(50.0),   // Small delegation
                    sol_to_lamports(500.0),  // Medium delegation
                    sol_to_lamports(5000.0), // Large delegation
                ],
                operator_fee_bps: 90, // 0.9% fee
            },
            // Test 2: Extreme delegation amounts
            SimConfig {
                operator_count: 20,
                mints: vec![MintConfig {
                    keypair: Keypair::new(),
                    weight: 2 * WEIGHT, // Double weight
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
                        weight: WEIGHT / 2, // Half weight
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
