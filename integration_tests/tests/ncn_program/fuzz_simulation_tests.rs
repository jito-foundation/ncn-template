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

    // Main simulation function that runs a full consensus cycle with the given configuration
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

        // Setup Node Consensus Network (NCN)
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
