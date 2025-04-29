# Simulation Test Detailed Guide

## Overview

The simulation test is a comprehensive test case that simulates a complete tip router system with multiple operators, vaults, and token types. It tests the entire flow from setup to voting and consensus reaching.

Note that at the bottem of these, there is a detailed explanations for each function mentioned in this guide

## Test Components

### Initial Setup

```rust
let mut fixture = TestBuilder::new().await;
```

This function initializes the test environment by:

1. Determining whether to run using BPF (Solana's Berkeley Packet Filter)
2. Setting up the program test environment with the TipRouter, Vault, and Restaking programs
3. Starting the test context that simulates the Solana runtime

after that we have

```rust
let mut tip_router_client = fixture.tip_router_client();
let mut vault_program_client = fixture.vault_client();
let mut restaking_client = fixture.restaking_program_client();

const OPERATOR_COUNT: usize = 13;
let mints = vec![
    (Keypair::new(), WEIGHT),           // TKN1
    (Keypair::new(), WEIGHT),           // TKN2
    (Keypair::new(), WEIGHT),           // TKN3
    (Keypair::new(), WEIGHT_PRECISION), // TKN4
];

let delegations = [
    1,
    sol_to_lamports(1000.0),
    sol_to_lamports(10000.0),
    sol_to_lamports(100000.0),
    sol_to_lamports(1000000.0),
    sol_to_lamports(10000000.0),
];
```

- getting ready by getting the clients we are going to use, and specifiying the number of operators (9 in this case)
- preper 4 different token types, these could be any SPL tokens, and you could have any number of them, also worth to mention that you (as the NCN admin) could change any and all of these tokens at any time, so you could change the number of them, the weight, etc.
- Defines various delegation amounts for testing
- Ranges from 1 lamport to 10M SOL

## Test Flow

### NCN Setup

```rust
let mut test_ncn = fixture.create_test_ncn().await?;
let ncn = test_ncn.ncn_root.ncn_pubkey;
```

- Creates a new NCN (Network Control Node)
- Stores the NCN public key for later use
- In order to understand this better, refeer to the section `Detailed Function Explanations` and see the first 4 function

### Operator and Vault Setup

before starting anything, you will need to register any operators and vaults you want to use, and handshake them with the NCN, in addition to that you will need to handshake the vaults with the operators that they will delegate to.

```rust
// Add operators
fixture.add_operators_to_test_ncn(&mut test_ncn, OPERATOR_COUNT, Some(100)).await?;

// Add vaults for each token type
fixture.add_vaults_to_test_ncn(&mut test_ncn, 3, Some(mints[0].0.insecure_clone())).await?; // TKN1
fixture.add_vaults_to_test_ncn(&mut test_ncn, 2, Some(mints[1].0.insecure_clone())).await?; // TKN2
fixture.add_vaults_to_test_ncn(&mut test_ncn, 1, Some(mints[2].0.insecure_clone())).await?; // TKN3
fixture.add_vaults_to_test_ncn(&mut test_ncn, 1, Some(mints[3].0.insecure_clone())).await?; // TKN4
```

- Adds 13 operators with 100 basis points fee
  - this is using restaking-program by Jito
- Creates vaults for each token type:
  - 3 TKN1 vaults
  - 2 TKN2 vaults
  - 1 TKN3 vault
  - 1 TKN4 vault
  - in addition to that, the vaults are handshaked with the NCN, and with the operators that they will delegate to
  - this is using vault-program by Jito

### Delegation Setup

In order for an operator to vote, it have to have some voting power, in this case the voting power is determined by how much delegation does it have. The delegation amount is not the voting power it self, but a factor of it, to get the voting power, you have to multiply the delegation amount by the weight of the token type.

```rust
for (index, operator_root) in test_ncn.operators.iter().take(OPERATOR_COUNT - 1).enumerate() {
    for vault_root in test_ncn.vaults.iter() {
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

- Assigns delegations to operators for each vault
- Uses different delegation amounts from the predefined list
- Skips the last operator (zero delegation operator), the reason is to test later that no delegatoin operators cannot vote

### ST Mints and vaults Registration

This step will keep track of each mint supported by this NCN and its weight. The reason behind it is to help later with taking snapshots for the system, this step will be even more important if the NCN decides to assign weights by token price using an oracle like switchboard, in this case instead of storing the weight here you can store the feed that is going to be used to fetch the price before each epoch.

```rust
let restaking_config_address = Config::find_program_address(&jito_restaking_program::id()).0;
let restaking_config = restaking_client.get_config(&restaking_config_address).await?;
let epoch_length = restaking_config.epoch_length();

fixture.warp_slot_incremental(epoch_length * 2).await.unwrap(); // after setting up operators and vault connections, there must be a wait time of a full epoch for the connections to get activated

// Register ST mints
for (mint, weight) in mints.iter() {
    tip_router_client
        .do_admin_register_st_mint(ncn, mint.pubkey(), *weight)
        .await?;
}

// Register vaults
for vault in test_ncn.vaults.iter() {
    let vault = vault.vault_pubkey;
    let (ncn_vault_ticket, _, _) = NcnVaultTicket::find_program_address(
        &jito_restaking_program::id(),
        &ncn,
        &vault,
    );
    tip_router_client.do_register_vault(ncn, vault, ncn_vault_ticket).await?;
}
```

- Warps time forward by 2 epoch lengths
- Registers each ST mint with its corresponding weight
- Registers each vault with the NCN
- These calls are using instructions from the NCN program that you will deploy

### Epoch Snapshot

#### Epoch State

Epoch state account will hold all the required information to tell the current stage of the voting cycle, it will keep trach of the prograss for many things like:

- setting the weights
- epoch snapshot
- operator snapshot
- voting progress
- is closing

as well as keep track of:

- was the tie breaker set
- slot when the consensus was reached
- vault count
- operator count
- epoch

```rust
fixture.add_epoch_state_for_test_ncn(&test_ncn).await?;
```

#### Admin Set Weights

This step is important to set the weights that are going to be used for this epoch, it is even more important if you are using an oracle to set the weights.

```rust
fixture.add_admin_weights_for_test_ncn(&test_ncn).await?;
```

#### Epoch Snapshot Taking

Here is where we take the snapshot for the epoch, This step will determine the voting power for each operator, and will be used to determine the winning ballot.

```rust
fixture.add_epoch_snapshot_to_test_ncn(&test_ncn).await?;
fixture.add_operator_snapshots_to_test_ncn(&test_ncn).await?;
fixture.add_vault_operator_delegation_snapshots_to_test_ncn(&test_ncn).await?;
fixture.add_ballot_box_to_test_ncn(&test_ncn).await?;
```

### Voting Process

Voting is a task that will be carried out by the operators, In the real world the operators will call an instruction on the onchain program (usually called `vote`) and they will provide there vote.

But before that, the NCN admin will need to initiate the voting process for the epoch by initializing the ballot box

```rust
fixture.add_ballot_box_to_test_ncn(&test_ncn).await?;
```

In this test case, we will call a helper function that will do the voting for us, this function will take the operator's vote and cast it

and it will be done by calling the `do_cast_vote` function, this function will take the operator's vote and store it in the ballot box.

```rust
let epoch = fixture.clock().await.epoch;

// Zero delegation operator votes Rainy
let zero_delegation_operator = test_ncn.operators.last().unwrap();
tip_router_client
    .do_cast_vote(
        ncn,
        zero_delegation_operator.operator_pubkey,
        &zero_delegation_operator.operator_admin,
        WeatherStatus::Rainy as u8,
        epoch,
    )
    .await?;

// Other operators vote Sunny
let weather_status = WeatherStatus::Sunny as u8;
// ... voting for first three operators ...

// Remaining operators vote Sunny
for operator_root in test_ncn.operators.iter().take(OPERATOR_COUNT - 1).skip(3) {
    tip_router_client
        .do_cast_vote(
            ncn,
            operator_root.operator_pubkey,
            &operator_root.operator_admin,
            weather_status,
            epoch,
        )
        .await?;
}
```

- First operator votes "Rainy", and it is also the zero-delegation operator, so his vote will not carry any weight
- All other operators vote "Sunny"
- Tests consensus reaching with majority voting

### Verification

```rust
let ballot_box = tip_router_client.get_ballot_box(ncn, epoch).await?;
assert!(ballot_box.has_winning_ballot());
assert!(ballot_box.is_consensus_reached());
assert_eq!(
    ballot_box.get_winning_ballot().unwrap().weather_status(),
    weather_status
);
```

- Verifies that:
  - A winning ballot exists
  - Consensus has been reached
  - The winning weather status is "Sunny"

### Cleanup

```rust
fixture.close_epoch_accounts_for_test_ncn(&test_ncn).await?;
```

- Closes all epoch-related accounts
- Cleans up test resources

## Key Test Aspects

1. **Multiple Token Types**: Tests the system with 4 different token types
2. **Varying Delegations**: Tests different delegation amounts
3. **Consensus Mechanism**: Verifies the voting and consensus reaching process
4. **Zero Delegation Handling**: Tests behavior with a zero-delegation operator
5. **Majority Voting**: Ensures the system correctly identifies the majority vote
6. **Account Management**: Tests proper creation and cleanup of all necessary accounts

## Expected Outcomes

1. All operators should be able to cast votes
2. The system should reach consensus despite one dissenting vote
3. The winning weather status should be "Sunny"
4. All accounts should be properly created and cleaned up

## Error Cases

The test implicitly verifies handling of:

- Multiple token types
- Various delegation amounts
- Zero delegation operators
- Majority vs minority voting
- Account initialization and cleanup

## Detailed Function Explanations

### `create_test_ncn()`

The code section from lines 260-345 contains several important functions that work together to set up the test environment:

#### `create_test_ncn()`

```rust
pub async fn create_test_ncn(&mut self) -> TestResult<TestNcn> {
    let mut restaking_program_client = self.restaking_program_client();
    let mut vault_program_client = self.vault_program_client();
    let mut tip_router_client = self.tip_router_client();

    vault_program_client.do_initialize_config().await?;
    restaking_program_client.do_initialize_config().await?;

    let ncn_root = restaking_program_client
        .do_initialize_ncn(Some(self.context.payer.insecure_clone()))
        .await?;

    tip_router_client.setup_tip_router(&ncn_root).await?;

    Ok(TestNcn {
        ncn_root: ncn_root.clone(),
        operators: vec![],
        vaults: vec![],
    })
}
```

This function is the entry point for creating a test NCN. It:

1. Gets clients for the restaking, vault, and tip router programs
2. Initializes configurations for both the vault and restaking programs
3. Creates a new NCN using the restaking program
4. Sets up the tip router with the newly created NCN
5. Returns a TestNcn struct containing the NCN root and empty lists for operators and vaults

#### `setup_tip_router()`

```rust
pub async fn setup_tip_router(&mut self, ncn_root: &NcnRoot) -> TestResult<()> {
    self.do_initialize_config(ncn_root.ncn_pubkey, &ncn_root.ncn_admin)
        .await?;

    self.do_full_initialize_vault_registry(ncn_root.ncn_pubkey)
        .await?;

    Ok(())
}
```

This function sets up the tip router by:

1. Initializing the configuration for the tip router
2. Setting up the vault registry
3. Both operations use the NCN's public key and admin keypair

#### `do_initialize_config()`

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

This function prepares the configuration by:

1. Airdropping 1 SOL to the payer account
2. Finding and airdropping 100 SOL to the account payer PDA
3. Getting the NCN admin's public key
4. Calling initialize_config with specific parameters:
   - 3 epochs before stall
   - 10 epochs after consensus before close
   - 10000 valid slots after consensus

#### `initialize_config()`

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

This function performs the actual configuration initialization by:

1. Finding the NCN config PDA address
2. Finding the account payer PDA address
3. Building an initialization instruction with all necessary parameters
4. Getting the latest blockhash
5. Processing the transaction with the NCN admin as the signer

The configuration parameters control important timing aspects of the system:

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

This function adds operators to the NCN by:

1. Creating each operator with the specified fee in basis points
2. Initializing the relationship between the NCN and each operator
3. Warming up the relationship (activating it) in both directions
4. Adding each operator to the TestNcn struct

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
    let mint_amount: u64 = sol_to_lamports(100_000_000.0);

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
        self.warp_slot_incremental(1).await.unwrap();

        test_ncn.vaults.push(vault_root);
    }

    Ok(())
}
```

This function creates vaults for the test by:

1. Setting up vault parameters with zero fees
2. Either using the provided token mint or generating a new one
3. Initializing each vault with the specified parameters
4. Creating the connection between the vault and the NCN
5. Adding each vault to the TestNcn struct

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

This function registers a staking token (ST) mint with:

1. Finding the vault registry address for the NCN
2. Finding the NCN config address
3. Using the payer as the admin
4. Calling the underlying admin_register_st_mint function with all parameters

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

This function initializes the epoch state by:

1. Warping time forward 1000 slots
2. Getting the current epoch
3. Initializing an epoch state for the NCN at the current epoch

### `add_admin_weights_for_test_ncn()`

```rust
pub async fn add_admin_weights_for_test_ncn(&mut self, test_ncn: &TestNcn) -> TestResult<()> {
    let mut tip_router_client = self.tip_router_client();

    let clock = self.clock().await;
    let epoch = clock.epoch;
    tip_router_client
        .do_full_initialize_weight_table(test_ncn.ncn_root.ncn_pubkey, epoch)
        .await?;

    let ncn = test_ncn.ncn_root.ncn_pubkey;
    let vault_registry = tip_router_client.get_vault_registry(ncn).await?;

    for entry in vault_registry.st_mint_list {
        if entry.is_empty() {
            continue;
        }

        let st_mint = entry.st_mint();
        tip_router_client
            .do_admin_set_weight(
                test_ncn.ncn_root.ncn_pubkey,
                epoch,
                *st_mint,
                entry.weight(),
            )
            .await?;
    }

    Ok(())
}
```

This function sets weights for each ST mint by:

1. Initializing a weight table for the current epoch
2. Getting the vault registry to find all registered ST mints
3. Setting the admin-defined weight for each ST mint

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

This function creates a ballot box for voting by:

1. Getting the current epoch
2. Initializing a ballot box for the NCN at the current epoch

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

This function casts a vote for a weather status by:

1. Finding addresses for all required accounts
2. Building a cast vote instruction with the operator and weather status
3. Processing the transaction with the operator admin as a signer

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
