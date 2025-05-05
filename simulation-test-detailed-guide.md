# Simulation Test Detailed Guide

## Table of Contents

1. [Overview](#overview)
1. [Prerequisites](#prerequisites)
1. [Test Components](#test-components)
1. [Test Flow](#test-flow)
   1. [NCN Setup](#1-ncn-setup)
   1. [NCN Configuration Management](#2-ncn-configuration-management)
   1. [Operator and Vault Setup](#operator-and-vault-setup)
   1. [Delegation Setup](#delegation-setup)
   1. [ST Mints and Vaults Registration](#st-mints-and-vaults-registration)
   1. [Epoch Snapshot](#epoch-snapshot)
   1. [Voting Process](#voting-process)
   1. [Verification](#verification)
   1. [Cleanup](#cleanup)
1. [Detailed Function Explanations](#detailed-function-explanations)
1. [Expected Outcomes](#expected-outcomes)
1. [Error Cases](#error-cases)

## Overview

The simulation test is a comprehensive test case that simulates a complete tip router system with multiple operators, vaults, and token types. It tests the entire flow from setup to voting and consensus reaching.

## Prerequisites

Before running the simulation test, ensure you have:

1. Set up the test ledger using `./tip-router-operator-cli/scripts/setup-test-ledger.sh`
1. Built the tip router program using `cargo build-sbf`
1. Set the correct Solana version (1.18.26 recommended)

## Test Components

### Initial Setup

The test begins with initializing the test environment:

```rust
let mut fixture = TestBuilder::new().await;
```

This function initializes the test environment by:

1. Determining whether to run using BPF (Solana's Berkeley Packet Filter)
1. Setting up the program test environment with the TipRouter, Vault, and Restaking programs
1. Starting the test context that simulates the Solana runtime

After that, the following code is executed:

```rust
let mut tip_router_client = fixture.tip_router_client();
let mut vault_program_client = fixture.vault_client();
let mut restaking_client = fixture.restaking_program_client();

const OPERATOR_COUNT: usize = 13;  // Number of operators to create for testing
let mints = vec![
    (Keypair::new(), WEIGHT),     // TKN1 with base weight
    (Keypair::new(), WEIGHT * 2), // TKN2 with double weight
    (Keypair::new(), WEIGHT * 3), // TKN3 with triple weight
    (Keypair::new(), WEIGHT * 4), // TKN4 with quadruple weight
];

let delegations = [
    1,                  // minimum delegation amount
    sol_to_lamports(1000.0),
    sol_to_lamports(10000.0),
    sol_to_lamports(100000.0),
    sol_to_lamports(1000000.0),
    sol_to_lamports(10000000.0),
];
```

This setup:

1. Initializes clients for each program
1. Defines 13 operators
1. Sets up 4 different token types with their respective weights:
   - TKN1: Base weight (WEIGHT)
   - TKN2: Double weight (WEIGHT * 2)
   - TKN3: Triple weight (WEIGHT * 3)
   - TKN4: Quadruple weight (WEIGHT * 4)
1. Defines various delegation amounts for testing, from minimal (1 lamport) to very large (10M SOL)

### 1. NCN Setup

```rust
// Create a Node Consensus Network (NCN)
let mut test_ncn = fixture.create_test_ncn().await?;
let ncn_pubkey = test_ncn.ncn_root.ncn_pubkey;
```

This code:

- Creates a new NCN (Network Coordination Node)
- Stores the NCN public key for later use
- For a detailed explanation of this process, refer to the "Detailed Function Explanations" section

### 2. Operator and Vault Setup

Before starting the voting process, the following steps are required:

1. Register operators and vaults
1. Establish handshakes between the NCN and operators
1. Establish handshakes between vaults and their delegated operators

Here is the code:

```rust
// Add operators - Creates OPERATOR_COUNT operators with a 100 bps (1%) fee
fixture.add_operators_to_test_ncn(&mut test_ncn, OPERATOR_COUNT, Some(100)).await?;

// Add vaults for each token type
fixture.add_vaults_to_test_ncn(&mut test_ncn, 3, Some(mints[0].0.insecure_clone())).await?; // Create 3 vaults for TKN1
fixture.add_vaults_to_test_ncn(&mut test_ncn, 2, Some(mints[1].0.insecure_clone())).await?; // Create 2 vaults for TKN2
fixture.add_vaults_to_test_ncn(&mut test_ncn, 1, Some(mints[2].0.insecure_clone())).await?; // Create 1 vault for TKN3
fixture.add_vaults_to_test_ncn(&mut test_ncn, 1, Some(mints[3].0.insecure_clone())).await?; // Create 1 vault for TKN4
```

This code:

- Adds 13 operators with a 100 basis points fee (1%) using the Jito restaking program
- Creates vaults for each token type with different amounts:
  - 3 TKN1 vaults (base weight)
  - 2 TKN2 vaults (double weight)
  - 1 TKN3 vault (triple weight)
  - 1 TKN4 vault (quadruple weight)
- Establishes connections between vaults, the NCN, and their delegated operators using the Jito vault program

### 3. Delegation Setup

An operator's voting power is determined by their delegation amount, which is multiplied by the weight of the token type.

```rust
// Each vault delegates different amounts to different operators based on the delegation amounts array
for (index, operator_root) in test_ncn
    .operators
    .iter()
    .take(OPERATOR_COUNT - 1)  // All operators except the last one
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
```

This code:

- Assigns delegations to all operators except the last one for each vault
- Uses different delegation amounts from the predefined list, cycling through them
- Skips the last operator to create a "zero delegation operator" for testing how operators without delegation are handled

### 4. ST Mints and Vaults Registration

This step tracks each mint supported by the NCN and its weight. This information is crucial for taking system snapshots, specially if the token price is used as the weight, in this case an oracle (like Switchboard) could be used to fetch token prices before each vote

```rust
// 3.a. Initialize the config for the ncn-program
tip_router_client
    .do_initialize_config(test_ncn.ncn_root.ncn_pubkey, &test_ncn.ncn_root.ncn_admin)
    .await?;

// 3.b Initialize the vault_registry - creates accounts to track vaults
tip_router_client
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
    tip_router_client
        .do_admin_register_st_mint(ncn_pubkey, mint.pubkey(), *weight)
        .await?;
}

// 3.d Register all the vaults in the ncn program
// This makes the vaults eligible for the tip routing system
for vault in test_ncn.vaults.iter() {
    let vault = vault.vault_pubkey;
    let (ncn_vault_ticket, _, _) = NcnVaultTicket::find_program_address(
        &jito_restaking_program::id(),
        &ncn_pubkey,
        &vault,
    );

    tip_router_client
        .do_register_vault(ncn_pubkey, vault, ncn_vault_ticket)
        .await?;
}
```

This code:

1. Initializes the NCN configuration
2. Sets up the vault registry to track supported vaults
3. Warps time forward by 2 epoch lengths to ensure all handshake relationships are active
4. Registers each ST mint with its corresponding weight:
   - TKN1: base weight (WEIGHT)
   - TKN2: double weight (WEIGHT * 2) 
   - TKN3: triple weight (WEIGHT * 3)
   - TKN4: quadruple weight (WEIGHT * 4)
5. Registers each vault with the NCN, connecting it to the token it supports

The weights play a crucial role in the voting system as they multiply the delegation amounts to determine voting power. This setup tests how different token weights affect voting outcomes.

### 5. Epoch Snapshot

#### Epoch State

The epoch state account is the reference to track the current phase of the voting cycle:

```rust
// 4.a. Initialize the epoch state - creates a new state for the current epoch
fixture.add_epoch_state_for_test_ncn(&test_ncn).await?;
```

This creates an epoch state account that tracks:

- Current stage of the voting cycle
- Progress of weight setting
- Epoch snapshot status
- Operator snapshot status
- Voting progress
- Closing status
- Tie breaker status
- Consensus slot
- Vault and operator counts
- Current epoch

#### Setting Weights for Current Epoch

```rust
// 4.b. Initialize the weight table - prepares the table that will track voting weights
let clock = fixture.clock().await;
let epoch = clock.epoch;
tip_router_client
    .do_full_initialize_weight_table(test_ncn.ncn_root.ncn_pubkey, epoch)
    .await?;

// 4.c. Take a snapshot of the weights for each ST mint
// This records the current weights for the voting calculations
tip_router_client
    .do_set_epoch_weights(test_ncn.ncn_root.ncn_pubkey, epoch)
    .await?;
```

This step:
1. Creates a weight table for the current epoch
2. Copies the weights from the vault registry to the weight table, locking them for this voting cycle
3. This is especially important when weights are dynamic (like token prices)

#### Taking Snapshots

```rust
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
```

This code:
1. Creates an epoch snapshot with aggregate data
2. Takes individual snapshots for each operator
3. Records all vault-to-operator delegations to determine voting power

#### Initialize Ballot Box

```rust
// 4.g. Initialize the ballot box - creates the voting container for this epoch
fixture.add_ballot_box_to_test_ncn(&test_ncn).await?;
```

This creates the ballot box where votes will be tallied.

### 6. Voting Process

Voting is performed by operators through an onchain program instruction. In this test, we simulate different operators voting for different weather statuses:

```rust
// Define which weather status we expect to win in the vote
let winning_weather_status = WeatherStatus::Sunny as u8;

// 5. Cast votes from operators
{
    let epoch = fixture.clock().await.epoch;

    let zero_delegation_operator = test_ncn.operators.last().unwrap();  // Operator with no delegations
    let first_operator = &test_ncn.operators[0];
    let second_operator = &test_ncn.operators[1];
    let third_operator = &test_ncn.operators[2];

    // Vote from zero_delegation_operator (should fail with an error since operators with zero delegations cannot vote)
    {
        let weather_status = WeatherStatus::Rainy as u8;

        // We expect this to fail since the operator has zero delegations
        let result = tip_router_client
            .do_cast_vote(
                ncn_pubkey,
                zero_delegation_operator.operator_pubkey,
                &zero_delegation_operator.operator_admin,
                weather_status,
                epoch,
            )
            .await;
        
        // Verify that voting with zero delegation returns an error
        assert!(result.is_err(), "Expected error when voting with zero delegation");
    }

    // First operator votes for Cloudy
    tip_router_client
        .do_cast_vote(
            ncn_pubkey,
            first_operator.operator_pubkey,
            &first_operator.operator_admin,
            WeatherStatus::Cloudy as u8,
            epoch,
        )
        .await?;
        
    // Second and third operators vote for Sunny (the expected winner)
    tip_router_client
        .do_cast_vote(
            ncn_pubkey,
            second_operator.operator_pubkey,
            &second_operator.operator_admin,
            winning_weather_status,
            epoch,
        )
        .await?;
    tip_router_client
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

        tip_router_client
            .do_cast_vote(
                ncn_pubkey,
                operator,
                &operator_root.operator_admin,
                winning_weather_status,
                epoch,
            )
            .await?;
    }
}
```

This code:

- Tests that an operator with zero delegation cannot vote (expects an error)
- Has the first operator vote for "Cloudy"
- Has all other operators vote for "Sunny"
- Tests consensus reaching with different votes but a clear majority

### 7. Verification

```rust
// 6. Verify voting results
let ballot_box = tip_router_client.get_ballot_box(ncn_pubkey, epoch).await?;
assert!(ballot_box.has_winning_ballot());
assert!(ballot_box.is_consensus_reached());
assert_eq!(
    ballot_box.get_winning_ballot().unwrap().weather_status(),
    winning_weather_status
);

// 7. Fetch and verify the consensus_result account
{
    let epoch = fixture.clock().await.epoch;
    let consensus_result = tip_router_client
        .get_consensus_result(ncn_pubkey, epoch)
        .await?;

    // Verify consensus_result account exists and has correct values
    assert!(consensus_result.is_consensus_reached());
    assert_eq!(consensus_result.epoch(), epoch);
    assert_eq!(consensus_result.weather_status(), winning_weather_status);

    // Get ballot box to compare values
    let ballot_box = tip_router_client.get_ballot_box(ncn_pubkey, epoch).await?;
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
```

This code verifies that:

- A winning ballot exists
- Consensus has been reached
- The winning weather status is "Sunny" as expected
- The consensus result account records the correct voting weights
- The voting system correctly handles operators with different delegation amounts and tokens with different weights

### 8. Cleanup

```rust
// 8. Close epoch accounts but keep consensus result
let epoch_before_closing_account = fixture.clock().await.epoch;
fixture.close_epoch_accounts_for_test_ncn(&test_ncn).await?;

// Verify that consensus_result account is not closed (it should persist)
{
    let consensus_result = tip_router_client
        .get_consensus_result(ncn_pubkey, epoch_before_closing_account)
        .await?;

    // Verify consensus_result account exists and has correct values
    assert!(consensus_result.is_consensus_reached());
    assert_eq!(consensus_result.epoch(), epoch_before_closing_account);
}
```

This code:

1. Records the current epoch before closing accounts
2. Closes all epoch-related accounts
3. Verifies that the consensus result account is not closed - it should persist despite other accounts being closed

## Key Test Aspects

1. **Multiple Token Types**: Tests the system with 4 different token types with varying weights
2. **Varying Delegations**: Tests different delegation amounts from minimal to very large
3. **Consensus Mechanism**: Verifies the voting and consensus reaching process
4. **Zero Delegation Handling**: Tests behavior with a zero-delegation operator
5. **Different Votes**: Tests the system with operators voting for different options
6. **Account Management**: Tests proper creation and cleanup of all necessary accounts

## Expected Outcomes

1. All operators with delegations should be able to cast votes
2. Operators with zero delegations should not be able to vote (should return an error)
3. The system should reach consensus with "Sunny" as the winning weather status
4. All accounts should be properly created and cleaned up
5. The consensus result account should persist after cleaning up other accounts

## Detailed Function Explanations

### `create_test_ncn()`

This function creates a new NCN account using the restaking program:

```rust
pub async fn create_test_ncn(&mut self) -> TestResult<TestNcn> {
    let mut restaking_program_client = self.restaking_program_client();

    // calls jito restaking-program
    let ncn_root = restaking_program_client
        .do_initialize_ncn(Some(self.context.payer.insecure_clone()))
        .await?;

    Ok(TestNcn {
        ncn_root: ncn_root.clone(),
        operators: vec![],
        vaults: vec![],
    })
}
```

This function:

1. Gets clients for the restaking, vault, and tip router programs
1. Initializes configurations for both the vault and restaking programs
1. Creates a new NCN using the restaking program
1. Sets up the tip router with the newly created NCN
1. Returns a TestNcn struct containing the NCN root and empty lists for operators and vaults

### `do_admin_register_st_mint()`

```rust
pub async fn do_admin_register_st_mint(
    &mut self,
    ncn: Pubkey,
    st_mint: Pubkey,
    weight: u128,
) -> TestResult<()> {
    let vault_registry =
        VaultRegistry::find_program_address(&jito_tip_router_program::id(), &ncn).0;

    let (ncn_config, _, _) =
        NcnConfig::find_program_address(&jito_tip_router_program::id(), &ncn);

    let admin = self.payer.pubkey();

    self.admin_register_st_mint(ncn, ncn_config, vault_registry, admin, st_mint, weight)
        .await
}
```

This function:

1. Finds the vault registry address for the NCN
1. Finds the NCN config address
1. Uses the payer as the admin
1. Calls the underlying admin_register_st_mint function with all parameters to register a token mint with the specified weight

### `do_initialize_config()`

```rust
pub async fn do_initialize_config(
    &mut self,
    ncn: Pubkey,
    ncn_admin: &Keypair,
) -> TestResult<()> {
    // Setup Payer
    self.airdrop(&self.payer.pubkey(), 1.0).await?;

    // Setup account payer
    let (account_payer, _, _) =
        AccountPayer::find_program_address(&jito_tip_router_program::id(), &ncn);
    self.airdrop(&account_payer, 100.0).await?;

    let ncn_admin_pubkey = ncn_admin.pubkey();
    self.initialize_config(ncn, ncn_admin, &ncn_admin_pubkey, 3, 10, 10000)
        .await
}
```

This function:

1. Airdrops 1 SOL to the payer account
1. Finds and airdrops 100 SOL to the account payer PDA
1. Gets the NCN admin's public key
1. Calls initialize_config with specific parameters:
   - 3 epochs before stall
   - 10 epochs after consensus before close
   - 10000 valid slots after consensus

### `initialize_config()`

```rust
pub async fn initialize_config(
    &mut self,
    ncn: Pubkey,
    ncn_admin: &Keypair,
    tie_breaker_admin: &Pubkey,
    epochs_before_stall: u64,
    epochs_after_consensus_before_close: u64,
    valid_slots_after_consensus: u64,
) -> TestResult<()> {
    let config = NcnConfig::find_program_address(&jito_tip_router_program::id(), &ncn).0;

    let (account_payer, _, _) =
        AccountPayer::find_program_address(&jito_tip_router_program::id(), &ncn);

    // calls the NCN program
    let ix = InitializeConfigBuilder::new()
        .config(config)
        .ncn(ncn)
        .ncn_admin(ncn_admin.pubkey())
        .account_payer(account_payer)
        .tie_breaker_admin(*tie_breaker_admin)
        .epochs_before_stall(epochs_before_stall)
        .epochs_after_consensus_before_close(epochs_after_consensus_before_close)
        .valid_slots_after_consensus(valid_slots_after_consensus)
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
```

This function:

1. Finds the NCN config PDA address
1. Finds the account payer PDA address
1. Builds an initialization instruction with all necessary parameters
1. Gets the latest blockhash
1. Processes the transaction with the NCN admin as the signer

The configuration parameters control important timing aspects:

- `epochs_before_stall`: Number of epochs before the system is considered stalled
- `epochs_after_consensus_before_close`: Number of epochs to wait after reaching consensus before closing
- `valid_slots_after_consensus`: Number of slots that are considered valid after reaching consensus

### `add_operators_to_test_ncn()`

```rust
pub async fn add_operators_to_test_ncn(
    &mut self,
    test_ncn: &mut TestNcn,
    operator_count: usize,
    operator_fees_bps: Option<u16>,
) -> TestResult<()> {
    let mut restaking_program_client = self.restaking_program_client();

    for _ in 0..operator_count {
        let operator_root = restaking_program_client
            .do_initialize_operator(operator_fees_bps)
            .await?;

        // ncn <> operator
        restaking_program_client
            .do_initialize_ncn_operator_state(
                &test_ncn.ncn_root,
                &operator_root.operator_pubkey,
            )
            .await?;
        self.warp_slot_incremental(1).await.unwrap();
        restaking_program_client
            .do_ncn_warmup_operator(&test_ncn.ncn_root, &operator_root.operator_pubkey)
            .await?;
        restaking_program_client
            .do_operator_warmup_ncn(&operator_root, &test_ncn.ncn_root.ncn_pubkey)
            .await?;

        test_ncn.operators.push(operator_root);
    }

    Ok(())
}
```

This function:

1. Creates each operator with the specified fee in basis points
1. Initializes the relationship between the NCN and each operator
1. Warms up the relationship (activating it) in both directions
1. Adds each operator to the TestNcn struct

### `add_vaults_to_test_ncn()`

```rust
pub async fn add_vaults_to_test_ncn(
    &mut self,
    test_ncn: &mut TestNcn,
    vault_count: usize,
    token_mint: Option<Keypair>,
) -> TestResult<()> {
    let mut vault_program_client = self.vault_program_client();
    let mut restaking_program_client = self.restaking_program_client();

    const DEPOSIT_FEE_BPS: u16 = 0;
    const WITHDRAWAL_FEE_BPS: u16 = 0;
    const REWARD_FEE_BPS: u16 = 0;

    // TODO: change this number to be general tokens
    let mint_amount: u64 = sol_to_lamports(100_000_000.0);

    // TODO: simplify this by always providing a token_mint keypair
    let should_generate = token_mint.is_none();
    let pass_through = if token_mint.is_some() {
        token_mint.unwrap()
    } else {
        Keypair::new()
    };

    for _ in 0..vault_count {
        let pass_through = if should_generate {
            Keypair::new()
        } else {
            pass_through.insecure_clone()
        };

        let vault_root = vault_program_client
            .do_initialize_vault(
                DEPOSIT_FEE_BPS,
                WITHDRAWAL_FEE_BPS,
                REWARD_FEE_BPS,
                9,
                &self.context.payer.pubkey(),
                Some(pass_through),
            )
            .await?;

        // vault <> ncn
        restaking_program_client
            .do_initialize_ncn_vault_ticket(&test_ncn.ncn_root, &vault_root.vault_pubkey)
            .await?;
        self.warp_slot_incremental(1).await.unwrap();
        restaking_program_client
            .do_warmup_ncn_vault_ticket(&test_ncn.ncn_root, &vault_root.vault_pubkey)
            .await?;
        vault_program_client
            .do_initialize_vault_ncn_ticket(&vault_root, &test_ncn.ncn_root.ncn_pubkey)
            .await?;
         // TODO: why are we not warming-up vault ncn ticket?
        self.warp_slot_incremental(1).await.unwrap();

        test_ncn.vaults.push(vault_root);
    }

    Ok(())
}
```

This function:

1. Sets up vault parameters with zero fees
1. Either uses the provided token mint or generates a new one
1. Initializes each vault with the specified parameters
1. Creates the connection between the vault and the NCN
1. Adds each vault to the TestNcn struct

### `add_epoch_state_for_test_ncn()`

```rust
pub async fn add_epoch_state_for_test_ncn(&mut self, test_ncn: &TestNcn) -> TestResult<()> {
    let mut tip_router_client = self.tip_router_client();

    // Not sure if this is needed
    self.warp_slot_incremental(1000).await?;

    let clock = self.clock().await;
    let epoch = clock.epoch;
    tip_router_client
        .do_intialize_epoch_state(test_ncn.ncn_root.ncn_pubkey, epoch)
        .await?;

    Ok(())
}
```

This function:

1. Warps time forward 1000 slots
1. Gets the current epoch
1. Initializes an epoch state for the NCN at the current epoch

### `add_weights_for_test_ncn()`

```rust
pub async fn add_weights_for_test_ncn(&mut self, test_ncn: &TestNcn) -> TestResult<()> {
    let mut tip_router_client = self.tip_router_client();

    let clock = self.clock().await;
    let epoch = clock.epoch;
    tip_router_client
        .do_full_initialize_weight_table(test_ncn.ncn_root.ncn_pubkey, epoch)
        .await?;

    tip_router_client
        .do_set_epoch_weights(test_ncn.ncn_root.ncn_pubkey, epoch)
        .await?;

    Ok(())
}
```

This function:

1. Initializes a weight table for the current epoch
1. Gets the vault registry to find all registered ST mints
1. Sets the admin-defined weight for each ST mint

### `add_ballot_box_to_test_ncn()`

```rust
pub async fn add_ballot_box_to_test_ncn(&mut self, test_ncn: &TestNcn) -> TestResult<()> {
    let mut tip_router_client = self.tip_router_client();

    let clock = self.clock().await;
    let epoch = clock.epoch;
    let ncn = test_ncn.ncn_root.ncn_pubkey;

    tip_router_client
        .do_full_initialize_ballot_box(ncn, epoch)
        .await?;

    Ok(())
}
```

This function:

1. Gets the current epoch
1. Initializes a ballot box for the NCN at the current epoch

### `do_cast_vote()`

```rust
pub async fn do_cast_vote(
    &mut self,
    ncn: Pubkey,
    operator: Pubkey,
    operator_admin: &Keypair,
    weather_status: u8,
    epoch: u64,
) -> TestResult<()> {
    let epoch_state =
        EpochState::find_program_address(&jito_tip_router_program::id(), &ncn, epoch).0;
    let ncn_config =
        NcnConfig::find_program_address(&jito_tip_router_program::id(), &ncn).0;
    let ballot_box =
        BallotBox::find_program_address(&jito_tip_router_program::id(), &ncn, epoch).0;
    let epoch_snapshot =
        EpochSnapshot::find_program_address(&jito_tip_router_program::id(), &ncn, epoch).0;
    let operator_snapshot =
        OperatorSnapshot::find_program_address(&jito_tip_router_program::id(),
                                              &operator, &ncn, epoch).0;

    let ix = CastVoteBuilder::new()
        .epoch_state(epoch_state)
        .config(ncn_config)
        .ballot_box(ballot_box)
        .ncn(ncn)
        .epoch_snapshot(epoch_snapshot)
        .operator_snapshot(operator_snapshot)
        .operator(operator)
        .operator_voter(operator_admin.pubkey())
        .weather_status(weather_status)
        .epoch(epoch)
        .instruction();

    let blockhash = self.banks_client.get_latest_blockhash().await?;
    self.process_transaction(&Transaction::new_signed_with_payer(
        &[ix],
        Some(&self.payer.pubkey()),
        &[&self.payer, operator_admin],
        blockhash,
    ))
    .await
}
```

This function:

1. Finds addresses for all required accounts
2. Builds a cast vote instruction with the operator and weather status
3. Processes the transaction with the operator admin as a signer

### `WeatherStatus` Enum

```rust
#[derive(Debug, Default, Clone, Copy, Zeroable, PartialEq, Eq)]
#[repr(C)]
pub enum WeatherStatus {
    /// Clear sunny weather
    #[default]
    Sunny = 0,
    /// Cloudy weather conditions
    Cloudy = 1,
    /// Rainy weather conditions
    Rainy = 2,
}
```

This enum represents different weather conditions that operators vote on:

- `Sunny`: The default, represented by 0
- `Cloudy`: Represented by 1
- `Rainy`: Represented by 2

The weather status serves as a simple test mechanism for operators to vote on different conditions.

### `BallotBox` Implementation

The `BallotBox` struct tracks votes and determines consensus:

```rust
pub struct BallotBox {
    /// The NCN account this ballot box is for
    ncn: Pubkey,
    /// The epoch this ballot box is for
    epoch: PodU64,
    /// Bump seed for the PDA
    bump: u8,
    /// Slot when this ballot box was created
    slot_created: PodU64,
    /// Slot when consensus was reached
    slot_consensus_reached: PodU64,
    /// Number of operators that have voted
    operators_voted: PodU64,
    /// Number of unique ballots
    unique_ballots: PodU64,
    /// The ballot that got at least 66% of votes
    winning_ballot: Ballot,
    /// Operator votes
    operator_votes: [OperatorVote; 256],
    /// Mapping of ballots votes to stake weight
    ballot_tallies: [BallotTally; 256],
}
```

Key methods include:

- `cast_vote`: Records a vote from an operator
- `tally_votes`: Calculates the winning ballot based on stake weight
- `is_consensus_reached`: Determines if consensus (66%) has been reached
- `get_winning_ballot`: Returns the ballot with majority stake

## Error Cases

The test implicitly verifies handling of:

- Multiple token types
- Various delegation amounts
- Zero delegation operators (should be rejected with an error)
- Majority vs minority voting
- Account initialization and cleanup
