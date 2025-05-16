#[cfg(test)]
mod tests {
    use jito_restaking_core::{config::Config, ncn_vault_ticket::NcnVaultTicket};
    use ncn_program_core::{ballot_box::WeatherStatus, constants::WEIGHT};

    use solana_sdk::{msg, signature::Keypair, signer::Signer};

    use crate::fixtures::{test_builder::TestBuilder, TestResult};

    // #[ignore = "20-30 minute test"]
    #[tokio::test]
    async fn simulation_test() -> TestResult<()> {
        // 0.a. Building the test environment
        let mut fixture = TestBuilder::new().await;
        // 0.b. Initialize the configuration for the staking and vault programs
        // you will not have to do that on mainnet, the programs will already be configured
        fixture.initialize_restaking_and_vault_programs().await?;

        let mut ncn_program_client = fixture.ncn_program_client();
        let mut vault_program_client = fixture.vault_client();
        let mut restaking_client = fixture.restaking_program_client();

        // 1. Preparing the test variables
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

        // 2. Initializing all the needed accounts using Jito's Staking and Vault programs
        // this step will initialize the NCN account, and all the operators and vaults accounts,
        // it will also initialize the handshake relationships between all the NCN components

        // 2.a. Initialize the NCN account using the Jito Restaking program
        let mut test_ncn = fixture.create_test_ncn().await?;
        let ncn_pubkey = test_ncn.ncn_root.ncn_pubkey;

        // 2.b. Initialize the operators using the Jito Restaking program, and initiate the
        //   handshake relationship between the NCN <> operators
        {
            for _ in 0..OPERATOR_COUNT {
                // Set operator fee to 100 basis points (1%)
                let operator_fees_bps: Option<u16> = Some(100);

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

        // 2.d. Vaults delegate stakes to operators
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
            for (mint, weight) in mints.iter() {
                ncn_program_client
                    .do_admin_register_st_mint(ncn_pubkey, mint.pubkey(), *weight)
                    .await?;
            }

            // 4.d Register all the vaults in the ncn program
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

            let first_operator = &test_ncn.operators[0];
            let second_operator = &test_ncn.operators[1];
            let third_operator = &test_ncn.operators[2];

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
                "✅ Consensus Result Verified - Weather Status: {}, Vote Weight: {}, Total Weight: {}",
                consensus_result.weather_status(),
                consensus_result.vote_weight(),
                consensus_result.total_vote_weight(),
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
