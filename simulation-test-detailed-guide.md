# Node Consensus Network (NCN) Tutorial: Building a Blockchain Consensus System

## Table of Contents

- [Introduction](#introduction)
- [NCN Components](#ncn-components)
  - [Vaults](#vaults)
  - [Operators](#operators)
  - [NCN](#ncn)
- [Current NCN Example](#current-ncn-example)
  - [Program Overview](#program-overview)
  - [Key Components](#key-components)
  - [Weather Status System](#weather-status-system)
  - [Consensus Mechanism](#consensus-mechanism)
- [Building and running the Simulation Test](#building-and-running-the-simulation-test)
  - [Environment Setup](#environment-setup)
  - [NCN Setup](#ncn-setup)
  - [Operators and Vaults Setup](#operators-and-vaults-setup)
  - [NCN Program Configuration](#ncn-program-configuration)
  - [Voting Process](#voting-process)
  - [Conclusion](#conclusion)

## Introduction

The Node Consensus Network (NCN) is a robust blockchain consensus system built on Solana that enables network participants to reach agreement on critical network decisions. This system leverages Jito's restaking infrastructure to create a secure, stake-weighted voting mechanism where operators with delegated tokens can vote on network parameters and states.

### Why NCN Matters

Decentralized networks require reliable mechanisms for participants to reach consensus without central authorities. The NCN solves this by:

1. Providing a secure voting framework where influence is proportional to stake
2. Supporting multiple token types with configurable voting weights
3. Creating verifiable, immutable records of consensus decisions
4. Establishing a foundation for network governance and parameter setting

## NCN Components

To run an NCN you will 1 or more of each of three different components to connect with each others; vaults, operators and operators admin.

### 1. Vaults

Vaults are accounts that hold tokens and delegate them to operators. They play a crucial role in the NCN by:

1. Holding tokens
2. Delegating stake to operators
3. Participating in voting

### 2. Operators

Operators are accounts that receive stake from vaults and participate in voting. They play a crucial role in the NCN by:

1. Receiving stake from vaults
2. Participating in voting
3. Creating a network of participants

### 3. NCN

The NCN is the core component of the NCN. it represents the onchain program that the NCN developer will have to build and deploy, and it is the focus of this tutorial. It is responsible for:

1. Holding the configuration
2. Holding the vault registry
3. Holding the epoch state

## Current NCN Example

In this tutorial we will build a simulation test for an NCN program that is already provided, building the whole NCN code in a tutorial would be too much, especially if you want to account for all the edge cases and security vulnerabilities. so we decided to provide the code, and make this tutorial focused on the simulation test instead.

By writing the simulation (which its code already provided as well) you will get to touch and feel the whole NCN cycle, from initializing the vaults and operators using Jitos' restaking and vault programs, to initializing the NCN program configurations, and going through the full voting process.

### Program Overview

The NCN Program facilitates consensus on weather status through a stake-weighted voting mechanism. It operates in epochs and utilizes a weight-based system to determine the influence of different operators in the consensus process. Consensus is reached when votes representing at least 66% of the total stake weight agree on the same ballot.

### Key Components

1. Global Accounts: these accounts will get initialized at the start of the program, and will only get updated if needed any point in the future:
   1. **Config**: Stores global program configuration including epochs before stall, epochs after consensus before close, and valid slots after consensus
   1. **Vault Registry**: Manages registered vaults and supported stake token mints
1. Per consensus cycle accounts: these accounts will get initialized at the start of each consensus cycle (which is per epoch for thsi example), and they usually get closed alttle bit after the consensus cycle is over
   1. **Weight Table**: Maintains weights for different stake tokens to determine their relative importance
   1. **Epoch State**: Tracks epoch-specific state including consensus status and account lifecycle
   1. **Ballot Box**: Handles voting on weather status with stake-weighted tallying
   1. **Epoch Snapshot**: Captures stake delegations at specific epochs for consistent voting weight
   1. **Consensus Result**: Stores the final consensus outcome for each epoch

### Weather Status System

The program uses a simple weather status system as the consensus target:

1. **Sunny (0)**: Clear sunny weather
2. **Cloudy (1)**: Cloudy weather conditions
3. **Rainy (2)**: Rainy weather conditions

Operators vote on these status values, and the program tallies votes based on stake weight to determine the consensus result.

### Consensus Mechanism

The consensus process follows these steps:

1. Operators cast votes with a specific weather status
2. Each vote's influence is weighted by the operator's stake weight
3. Votes are tallied in the ballot box
4. Consensus is reached when a weather status receives ≥66% of the total stake weight
5. The consensus result is recorded with details about the winning status, vote weight, and timing

## Onchain program code

The onchain program is written with vanilla rust, and it is made of a number of instructions that could be called to perform the different actions required to run the NCN. You can find all the instrctions code inside `/program` folder, and `/core` folder contains the core logic that is shared between the instructions.

### Overview of the onchain instructions

1. Admin Instructions: these instructions are used to initialize the program, register tokens, and configure the program
   1. `admin_initialize_config`: initializes the program configuration
   1. `admin_register_st_mint`: registers a new supported token (ST) mint with the program
   1. `admin_set_new_admin`: sets a new admin for the program
   1. `admin_set_parameters`: sets the parameters for the program
   1. `admin_set_st_mint`: sets a new supported token (ST) mint with the program
   1. `admin_set_tie_breaker`: sets the tie breaker for the program
   1. `admin_set_weight`: sets the weight for a supported token (ST) mint
1. Keeper Instructions: these instructions are used to keep the program in check, they are premissenles so anyone can call them
   simulation-test-detailed-guide.md
   1. `initialize_epoch_state`: initializes the epoch state
   1. `initialize_vault_registry`: initializes the vault registry
   1. `realloc_vault_registry`: reallocates the vault registry
   1. `initialize_weight_table`: initializes the weight table
   1. `realloc_weight_table`: reallocates the weight table
   1. `initialize_epoch_snapshot`: initializes the epoch snapshot
   1. `initialize_operator_snapshot`: initializes the operator snapshot
   1. `set_epoch_weights`: sets the weights for the epoch
   1. `snapshot_vault_operator_delegation`: snapshots the vault operator delegation
   1. `initialize_ballot_box`: initializes the ballot box
   1. `realloc_ballot_box`: reallocates the ballot box
   1. `register_vault`: registers a new vault
   1. `close_epoch_account`: closes the epoch account
1. Operator Instruction: There is only one instruction that each operator will have to call each consensus cycle, yes you guesed it, `cast_vote` instruction

For more details you can always check the code, or check the API docs [here](put a link here)

## Building and running the Simulation Test

The simulation test is a comprehensive test case that simulates a complete NCN (Node Consensus Network) system with multiple operators, vaults, and token types. It tests the entire flow from setup to voting and consensus reaching. The system uses Jito's restaking infrastructure and custom voting logic to coordinate network participants.

## Prerequisites

Before running the simulation test, ensure you have:

1. Set up the test ledger using `./scripts/setup-test-ledger.sh`
1. Built the NCN program using `cargo build-sbf --manifest-path program/Cargo.toml --sbf-out-dir integration_tests/tests/fixtures`
1. Set the correct Solana version (2.2.6 recommended) and cargo version (1.81 or above)

## Building the Simulation Test

### 1. Create a new file

you can start on a blank file, and copy paste the code provided below to run the test, if you decide to do so, create a new file inside `integration_tests/tests` folder, and name it `simulation_test_new.rs` and add this code to it:

```rs
#[cfg(test)]
mod tests {
    use crate::fixtures::{test_builder::TestBuilder, TestResult};
    use jito_restaking_core::{config::Config, ncn_vault_ticket::NcnVaultTicket};
    use ncn_program_core::{ballot_box::WeatherStatus, constants::WEIGHT};
    use solana_sdk::{msg, signature::Keypair, signer::Signer};

    #[tokio::test]
    async fn simulation_test_new() -> TestResult<()> {
      // YOUR CODE WILL GO HERE
        Ok(())
    }
}

```

Now you will have to import it inside the `integration_tests/tests/mod.rs` file, so it can be run as a test, you can do it by adding this to the file:

```rs
mod simulation_test_new;
```

now to run the test, you can use the following command:

```bash
SBF_OUT_DIR=integration_tests/tests/fixtures cargo test -p ncn-program-integration-tests --test tests simulation_test_new
```

the command above will run only the new test, you can run all the test if you want by removing the part after -p

which it will pass for sure because there is nothing there yet, here is the expected output:

```bash
running 1 test
test ncn_program::simulation_test_new::tests::simulation_test_new ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 54 filtered out; finished in 0.00s
```

### 2. Environment Setup

The first thing to do is to create the test builder, which we will call `fixture`.

```rust
let mut fixture = TestBuilder::new().await;
```

After that we will initialize the restaking and vault programs, notice that we are doing this here becuase we are testing locally, in the case of testing on mainnet or devnet, you will not have to run this funtion

```rust
fixture.initialize_restaking_and_vault_programs().await?;
```

finally let's prepare some variables that we will use later in the test

```rust
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
```

This code:

1. store the client that we will interact with.
2. defines the number of operators to create for testing.
3. defines the mints and their weights.
4. defines the delegation amounts.

### 3. NCN Setup

This step will initialize the NCN account using the restaking program by Jito, `create_test_ncn` will call an instruction in the restaking program to create the NCN account

```rust
let mut test_ncn = fixture.create_test_ncn().await?;
let ncn_pubkey = test_ncn.ncn_root.ncn_pubkey;
```

This step:

- Creates a new Node Consensus Network (NCN) using Jito's restaking infrastructure
- Stores the NCN public key for future operations

At this point, if you run the test, you will see some output from the transactions that are going to run here, try it out.

### 4. Operators and Vaults Setup

The Operators and Vaults setup phase is critical to the simulation as it establishes the network of participants and their relationships. This creates the foundation for the consensus and voting mechanisms being tested.

#### 4.1 Operator Creation and NCN Connection

```rust
fixture
    .add_operators_to_test_ncn(&mut test_ncn, OPERATOR_COUNT, Some(100))
    .await?;
```

This step will call a couple of instructions in Jito restaking program to:

- Creates 13 operator accounts using Jito's restaking program
- Sets each operator's fee to 100 basis points (1%)
- Establishes a bidirectional handshake between each operator and the NCN

The handshake process involves:

1. Creating operator accounts with their respective admin keypairs
2. Initializing the NCN-operator relationship state using `do_initialize_ncn_operator_state`
3. Warming up the NCN-to-operator connection using `do_ncn_warmup_operator`
4. Warming up the operator-to-NCN connection using `do_operator_warmup_ncn`

These bidirectional relationships are essential for the security model, ensuring operators can only participate in voting if they have a valid, active connection to the NCN.

#### 4.2 Vault Creation for Different Token Types

```rust
// 2.c. Initialize vaults for each token type
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
```

This step calls a couple of instructions in Jito vault program and Jito restaking program to create a total of 7 vaults distributed across 4 different token types:

- 3 vaults for TKN1 (base weight)
- 2 vaults for TKN2 (double weight)
- 1 vault for TKN3 (triple weight)
- 1 vault for TKN4 (quadruple weight)

For each vault, the system:

1. Initializes a vault account via the vault program with zero fees (important for testing)
2. Creates a vault mint (token) if not provided directly
3. Establishes a bidirectional handshake between the vault and the NCN:
   - Initializes an NCN-vault ticket using `do_initialize_ncn_vault_ticket` which will call a specific instruction in Jito restaking program to do that
   - Warms up the ticket using `do_warmup_ncn_vault_ticket` which will call a specific instruction in Jito restaking program to do that
   - Creates a vault-NCN ticket using `do_initialize_vault_ncn_ticket` which will call specific instruction in Jito vault program to do that
   - Warms up the vault-NCN ticket using `do_warmup_vault_ncn_ticket` which will call a specific instruction in Jito vault program to do that
   - Advances slots to ensure the relationship activates
4. Establishes a bidirectional handshake between each vault and all the operators:
   - Initializes an operator-vault ticket using `do_initialize_operator_vault_ticket` which will call a specific instruction in Jito restaking program to do that
   - Warms up the operator-vault ticket using `do_warmup_operator_vault_ticket` which will call a specific instruction in Jito restaking program to do that
   - Initializes the vault-operator delegation using `do_initialize_vault_operator_delegation` which will call a specific instruction in Jito vault program to do that.
     - note that no delegation will happen at this point, this is just establishing the relationship.

The distribution of vaults across different token types enables testing how the system handles voting power with different token weights and concentrations.

#### 4.3 Delegation Setup

This is where vaults delegate stakes to operators, again this is going to call a specific instruction in Jito vault program to do that.

```rust
// 2.d. Vaults delegate stakes to operators
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
```

The delegation process is where voting power is established. Each vault delegates tokens to operators, which determines:

1. How much voting power each operator has
2. How token weights multiply that power
3. The distribution of influence across the network

Key aspects of the delegation setup:

- Every vault delegates to every operator (except the last one for this example)
  - Note that vaults can choose whom to delegate to, they don't have to delegate to all operators
- Delegation amounts cycle through the `delegations` array to test different scenarios
- The last operator intentionally receives zero delegation to test the system's handling of operators without stake
- The delegation is performed directly through the vault program using `do_add_delegation` which will call a specific instruction in the vault program to do that

#### 4.4 Delegation Architecture and Voting Power Calculation

The delegation architecture follows a multiplication relationship:

- Voting power = Delegation amount × Token weight
- Each operator accumulates voting power from all vaults that delegate to them
- For an operator with multiple delegations, the total voting power is the sum of all delegations multiplied by their respective token weights

For example:

- If Vault1 (TKN1, weight=W) delegates 100 tokens to OperatorA, the voting power is 100×W
- If Vault2 (TKN2, weight=2W) delegates 50 tokens to OperatorA, the additional voting power is 50×2W
- OperatorA's total voting power would be (100×W) + (50×2W) = 200W

This distributed delegation model enables testing complex scenarios where:

- Operators have different levels of influence
- Tokens with higher weights have proportionally more impact
- The distribution of delegations affects consensus outcomes

The deliberate omission of delegation to the last operator creates a control case to verify that operators with zero stake cannot influence the voting process, which is a critical security feature.

You can run the test now and see the output.

### 5. NCN Program Configuration

All the work above is using the Jito restaking program and Jito vault program, now we will start using the NCN program that you will have to deploy.

The NCN Program Configuration phase establishes the on-chain infrastructure necessary for the voting and consensus mechanisms. This includes setting up configuration parameters, creating data structures, and registering the token types and vaults that will participate in the system.

#### 5.1 Program Configuration Initialization

```rust
// 3.a. Initialize the config for the ncn-program
ncn_program_client
    .do_initialize_config(test_ncn.ncn_root.ncn_pubkey, &test_ncn.ncn_root.ncn_admin)
    .await?;
```

This step initializes the core configuration for the NCN program with critical parameters:

- **NCN Admin**: The authority that can modify configuration settings
- **Epochs Before Stall**: How many epochs before a non-completed consensus cycle is considered stalled (default: 3)
- **Epochs After Consensus Before Close**: How long to wait after consensus before closing epoch data (default: 10)
- **Valid Slots After Consensus**: How many slots votes are still accepted after consensus is reached (default: 10000)

Under the hood, this creates a `NcnConfig` account that stores these parameters and serves as the authoritative configuration for this NCN instance.
check out the config struct [here](#config)

#### 5.2 Vault Registry Initialization

The vault registery account is a big one, so it is not possible to initiate it in one call due to solana network limitation, so we will have to call the NCN program multiple times to get to the full size, the first call will be an init call to the instruction `admin_initialize_vault_registry`, after that we will call a realoc instruction `admin_realloc_vault_registry` to increase the size of the account, this will be done in a loop until the account is the correct size.

the realoc will take care of assigning the default values to the vault registry account once the desirable size is reached, and in our example, we will do that by calling one function `do_full_initialize_vault_registry`, if you want to learn more about this, you can check the API docs, or the source code

```rust
// 3.b Initialize the vault_registry - creates accounts to track vaults
ncn_program_client
    .do_full_initialize_vault_registry(test_ncn.ncn_root.ncn_pubkey)
    .await?;
```

The vault registry is a critical data structure that:

- Tracks all supported vault accounts
- Maintains the list of supported token mints (token types)
- Records the weight assigned to each token type
- Serves as the source of truth for vault and token configurations

Note that this is only initilizeing the vault registry, the vaults and the supported tokens will be registered in the next steps.

check out the vault registry struct [here](#vaultregistry)

#### 5.3 Activating Relationships with Time Advancement

```rust
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
```

This section:

1. Retrieves the epoch length from the restaking program configuration
2. Advances the simulation time by two full epochs
3. Ensures all handshake relationships between NCN, operators, and vaults become active

The time advancement is necessary because Jito's restaking infrastructure uses an activation period for security. This prevents malicious actors from quickly creating and voting with fake operators or vaults by enforcing a waiting period before they can participate.

#### 5.4 Token Registration and Weight Assignment

Now it is time to register the supported tokens with the NCN program and assign weights to each mint for voting power calculations.

```rust
// 3.c. Register all the ST (Support Token) mints in the ncn program
// This assigns weights to each mint for voting power calculations
for (mint, weight) in mints.iter() {
    ncn_program_client
        .do_admin_register_st_mint(ncn_pubkey, mint.pubkey(), *weight)
        .await?;
}
```

This step registers each Supported Token (ST) mint with the NCN program and assigns the appropriate weight:

- Each token mint (TKN1, TKN2, etc.) is registered with its corresponding weight
- The weights determine the voting power multiplier for delegations in that token
- Only the NCN admin has the authority to register tokens, ensuring trust in the system
- Registration involves updating the vault registry with each token's data
- The NCN admin can update the weights of the tokens at any time, which will affect the voting power of the delegations in the next consensus cycle

The weight assignment is fundamental to the design, allowing different tokens to have varying influence on the voting process based on their economic significance or other criteria determined by the NCN administrators.

Good to know that in real life examples, NCNs will probably want to have to set the token weights based on the token's price or market cap, to do so you will have to use an oracle to get the price of the token and then set the weight based on that, in this case you will have to store the feed of the price in this step instead of the weight.

#### 5.5 Vault Registration

Registering a vault is a premissionless operation, the reason is the admin has already gave premission to the vault to be part of the NCN in the vault registerition step earlier, so this step is just to register the vault in the NCN program.

```rust
// 3.d Register all the vaults in the ncn program
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
```

The final configuration step registers each vault with the NCN program:

1. For each vault created earlier, the system finds its NCN vault ticket PDA (Program Derived Address)
2. The vault is registered in the NCN program's vault registry
3. This creates the association between the vault and its supported token type
4. The registration enables the NCN program to track vault delegations for voting power calculation

This registration process establishes the complete set of vaults that can contribute to the voting system, creating a closed ecosystem of verified participants.

#### 5.6 Architecture Considerations

The NCN program configuration establishes a multi-layered security model:

1. **Authentication Layer**: Only the NCN admin can initialize configuration and register tokens
2. **Relationship Layer**: Only vaults and operators with established, active handshakes can participate
3. **Time Security Layer**: Enforced waiting periods prevent quick creation and use of malicious actors
4. **Registry Layer**: All participants must be registered and tracked in on-chain registries

This layered approach ensures the integrity of the voting system by validating the identity and relationships of all participants before they can influence the consensus process.

The configuration phase completes the preparation of the system's infrastructure, setting the stage for the actual voting mechanics to begin in subsequent phases.

### 6. Epoch Snapshot and Voting Preparation

The Epoch Snapshot and Voting Preparation phase is where the system captures the current state of all participants and prepares the infrastructure for voting. This is an essential component of the architecture as it ensures voting is based on a consistent, verifiable snapshot of the network state at a specific moment in time.

The upcoming part a keeper task (except for the voting part), which means that it is premissionless and can be done by anyone.

#### 6.1 Epoch State Initialization

```rust
// 4.a. Initialize the epoch state for the current epoch
fixture.add_epoch_state_for_test_ncn(&test_ncn).await?;
```

This step initializes the **Epoch State** for the current consensus cycle:

- It creates an `EpochState` account tied to the specific NCN and epoch.
- This account tracks the progress through each stage of the consensus cycle.
- It maintains flags for each phase (weight setting, snapshot taking, voting, closing).
- The epoch state provides protection against out-of-sequence operations.
- It stores metadata like the current epoch, slot information, and participant counts.

Once initialized, the `EpochState` account becomes the authoritative record of where the system is in the voting process, preventing operations from happening out of order or in duplicate.

you can take a look at the epoch state struct [here](#epochaccountstatus)

#### 6.2 Weight Table Initialization and Population

```rust
// 4.b. Initialize the weight table to track voting weights
let clock = fixture.clock().await;
let epoch = clock.epoch;
ncn_program_client
    .do_full_initialize_weight_table(test_ncn.ncn_root.ncn_pubkey, epoch)
    .await?;

// 4.c. Take a snapshot of weights for each token mint
ncn_program_client
    .do_set_epoch_weights(test_ncn.ncn_root.ncn_pubkey, epoch)
    .await?;
```

The weight table mechanism handles the token weights for the current epoch in two stages:

1.  **Weight Table Initialization**:
    - Creates a [`WeightTable`](#weighttable) account for the specific epoch using `do_full_initialize_weight_table`. This may involve multiple calls internally to allocate sufficient space.
    - Allocates space based on the number of supported tokens registered in the [`VaultRegistry`](#vaultregistry).
    - Links the table to the NCN and current epoch.
    - Initializes the table structure with empty entries.

2.  **Weight Setting**:
    - Populates the [`WeightTable`](#weighttable) by calling `do_set_epoch_weights`
    - Copies the current weights from the [`VaultRegistry`](#vaultregistry) to the epoch-specific `WeightTable`.
    - "Freezes" these weights for the duration of the consensus cycle.
    - Updates the [`EpochState`](#epochaccountstatus) to mark weight setting as complete.
    - Creates an immutable record of token weights that will be used for voting.

This two-step process is critical for the integrity of the system as it:

- Creates a permanent record of weights at the time voting begins.
- Prevents weight changes during a consensus cycle from affecting ongoing votes.
- Allows transparent verification of the weights used for a particular vote.
- Enables historical auditing of how weights changed over time.

#### 6.3 Epoch Snapshot Creation

```rust
// 4.d. Take the epoch snapshot
fixture.add_epoch_snapshot_to_test_ncn(&test_ncn).await?;
```

The epoch snapshot captures the aggregate state of the entire system:

- Creates an [`EpochSnapshot`](#epochsnapshot) account for the NCN and epoch.
- Records the total number of operators and vaults expected to participate.
- Captures the total potential stake weight across all participants (initialized to zero).
- Stores important metadata such as the snapshot creation slot.
- Serves as the reference point for total voting power calculations, acting as the denominator for consensus thresholds.

#### 6.4 Operator Snapshots

```rust
// 4.e. Take snapshots for all operators
fixture
    .add_operator_snapshots_to_test_ncn(&test_ncn)
    .await?;
```

This step creates an individual snapshot for each operator in the system:

- For each operator, it creates an [`OperatorSnapshot`](#operatorsnapshot) account linked to the operator, NCN, and epoch.
- Records the operator's total delegated stake weight at this moment (initialized to zero).
- Captures the expected number of vault delegations for the operator.
- Verifies the operator has active handshakes with the NCN.
- Validates the operator's eligibility to participate in voting.

These snapshots establish each operator's baseline for the current epoch. The actual voting power will be populated in the next step based on individual delegations. This ensures that later delegation changes cannot alter voting weight once the snapshot phase is complete.

#### 6.5 Vault-Operator Delegation Snapshots

```rust
// 4.f. Record all vault-to-operator delegations
fixture
    .add_vault_operator_delegation_snapshots_to_test_ncn(&test_ncn)
    .await?;
```

This crucial step iterates through each active vault-to-operator delegation and records its contribution to the operator's voting power:

- For each valid delegation found in the Jito Vault program:
    - Retrieves the corresponding token weight from the epoch's [`WeightTable`](#weighttable).
    - Calculates the weighted stake for that delegation (delegation amount * token weight).
    - Updates the relevant [`OperatorSnapshot`](#operatorsnapshot) by adding the calculated stake weight.
    - Stores detailed information about the weighted delegation within the [`OperatorSnapshot`](#operatorsnapshot)'s `vault_operator_stake_weight` array.
    - Increments the total stake weight in the global [`EpochSnapshot`](#epochsnapshot).
    - Creates a [`VaultOperatorDelegationSnapshot`](#vaultoperatordelegationsnapshot) account for detailed auditing.

These granular snapshots serve multiple purposes:

- They populate the [`OperatorSnapshot`](#operatorsnapshot) accounts with the actual stake weights used for voting.
- They update the [`EpochSnapshot`](#epochsnapshot) with the total voting power present in the system for this epoch.
- They provide detailed audit trails of exactly where each operator's voting power originates.
- They enable verification of correct weight calculation for each delegation.
- They prevent retroactive manipulation of the voting power distribution.

#### 6.6 Ballot Box Initialization

```rust
// 4.g. Initialize the ballot box for collecting votes
fixture.add_ballot_box_to_test_ncn(&test_ncn).await?;
```

The final preparation step creates the ballot box:

- Initializes a [`BallotBox`](#ballotbox) account linked to the NCN and epoch using `do_full_initialize_ballot_box`. Similar to the weight table, this may require multiple allocation calls internally.
- Creates arrays to track operator votes ([`OperatorVote`](#operatorvote)) and ballot tallies ([`BallotTally`](#ballottally)).
- Sets up the data structures for recording and counting votes.
- Prepares the consensus tracking mechanism.
- Links the ballot box to the [`EpochState`](#epochaccountstatus) for progress tracking.

The [`BallotBox`](#ballotbox) becomes the central repository where all votes are recorded and tallied during the voting process. It is designed to efficiently track:

- Which operators have voted and what they voted for.
- The cumulative stake weight behind each voting option (ballot).
- The current winning ballot (if any).
- Whether consensus has been reached.

#### 6.7 Architecture and Security Considerations

The snapshot system implements several key architectural principles:

1.  **Point-in-Time Consistency**: All snapshots capture the system state relative to the start of the epoch, creating a consistent view based on frozen weights and delegations present at that time.
2.  **Immutability**: Once taken and populated, snapshots cannot be modified, ensuring the integrity of the voting weights used.
3.  **Layered Verification**: The system enables verification at multiple levels:
    - Aggregate level (`EpochSnapshot`)
    - Participant level (`OperatorSnapshot`)
    - Relationship level (individual weighted delegations within `OperatorSnapshot`, optionally `VaultOperatorDelegationSnapshot`)
4.  **Defense Against Time-Based Attacks**: By freezing the state (weights and relevant delegations) before voting begins, the system prevents:
    - Late stake additions influencing outcomes within the *current* epoch.
    - Strategic withdrawals affecting voting power *after* the snapshot.
    - Any form of "stake voting power front-running" within the epoch.
5.  **Separation of State and Process**:
    - The state (snapshots, weights) is captured separately from the process (voting).
    - This clear separation simplifies reasoning about the system.
    - It enables more effective testing and verification.

The comprehensive snapshot approach ensures that voting occurs on a well-defined, verifiable view of the network's state, establishing a solid foundation for the actual voting process to follow.

### 7. Voting Process

The Voting Process is the core functionality of the NCN system, where operators express their preferences on the network state (represented by the "weather status" in this simulation). This process leverages the infrastructure and snapshots created in previous steps to ensure secure, verifiable, and stake-weighted consensus.

#### 7.1 Setting the Expected Outcome

```rust
// Define the expected winning weather status
let winning_weather_status = WeatherStatus::Sunny as u8;
```

For testing purposes, the system defines an expected outcome (`WeatherStatus::Sunny`). In a production environment, the winning outcome would be determined organically through actual operator votes based on real-world data or criteria. The weather status enum (`Sunny`, `Cloudy`, `Rainy`) serves as a simplified proxy for any on-chain decision that requires consensus.

#### 7.2 Casting Votes from Different Operators

```rust
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
}
```

This section demonstrates the system's ability to handle diverse voting preferences using the `do_cast_vote` helper, which calls the `cast_vote` instruction:

1.  The first operator votes for "Cloudy" (representing a minority view).
2.  The second and third operators vote for "Sunny" (the presumed majority view).
3.  Each `do_cast_vote` call invokes the NCN program with the operator's choice and admin signature.

Under the hood, each vote triggers several key operations within the `cast_vote` instruction:

- **Verification**:
    - Verifies the operator admin's signature.
    - Checks that the operator hasn't already voted in this epoch using the [`BallotBox`](#ballotbox).
    - Retrieves the operator's [`OperatorSnapshot`](#operatorsnapshot) to confirm eligibility and get its total stake weight.
    - Ensures the [`EpochState`](#epochaccountstatus) indicates voting is currently allowed.
- **Recording**:
    - Records the vote details (operator, slot, stake weight, ballot choice) in the `operator_votes` array within the [`BallotBox`](#ballotbox).
    - Marks the operator as having voted.
- **Tallying**:
    - Finds or creates a [`BallotTally`](#ballottally) for the chosen weather status in the `ballot_tallies` array.
    - Adds the operator's full stake weight (from the snapshot) to this tally.
    - Increments the raw vote count for this tally.
- **Consensus Check**:
    - Compares the updated tally's stake weight against the total stake weight recorded in the [`EpochSnapshot`](#epochsnapshot).
    - If the tally now exceeds the consensus threshold (e.g., 66%), it marks consensus as reached in the [`BallotBox`](#ballotbox) and records the current slot.

#### 7.3 Establishing Consensus Through Majority Voting

```rust
    // All remaining operators vote for Sunny to form a majority
    for operator_root in test_ncn.operators.iter().take(OPERATOR_COUNT).skip(3) {
        ncn_program_client
            .do_cast_vote(
                ncn_pubkey,
                operator_root.operator_pubkey,
                &operator_root.operator_admin,
                winning_weather_status,
                epoch,
            )
            .await?;
    }
}
```

To establish a clear consensus in the test, the remaining eligible operators (excluding the first three and the one with zero delegation) all vote for the "Sunny" option. This accumulation of stake weight behind "Sunny" surpasses the required threshold.

The consensus mechanism works as follows:

1.  The system maintains a running [`BallotTally`](#ballottally) for each unique option voted on.
2.  After each vote, it recalculates the total stake weight supporting the voted option.
3.  It compares this stake weight to the total stake weight available in the [`EpochSnapshot`](#epochsnapshot).
4.  If an option's stake weight reaches the consensus threshold (e.g., >= 66%), the system:
    - Marks that `Ballot` as the `winning_ballot` in the [`BallotBox`](#ballotbox).
    - Records the current `slot` in `slot_consensus_reached`.
    - Updates the [`EpochState`](#epochaccountstatus).
    - Creates a persistent [`ConsensusResult`](#consensusresult) account (discussed in Verification).
5.  Consensus requires a supermajority to ensure decisions have strong, verifiable support across the network's weighted stake.

#### 7.4 Vote Processing Architecture

When an operator casts a vote via the `cast_vote` instruction, the system performs several critical operations:

1.  **Authentication**: Verifies the transaction is signed by the correct `operator_admin` keypair associated with the `operator` account.
2.  **Authorization & Preconditions**: Confirms that:
    - The operator exists, is registered with the NCN, and has an active [`OperatorSnapshot`](#operatorsnapshot) for the current `epoch`.
    - The operator has not already voted in this epoch (checked via [`BallotBox`](#ballotbox)).
    - The operator has non-zero stake weight in their [`OperatorSnapshot`](#operatorsnapshot).
    - The [`EpochState`](#epochaccountstatus) confirms that the snapshotting phase is complete and voting is open.
3.  **Vote Recording**:
    - Locates an empty slot or confirms the operator hasn't voted in the `operator_votes` array within the [`BallotBox`](#ballotbox).
    - Stores the `operator` pubkey, current `slot`, the operator's total `stake_weights` (from [`OperatorSnapshot`](#operatorsnapshot)), and the index corresponding to the chosen ballot within the `ballot_tallies` array.
    - Increments the `operators_voted` counter in the [`BallotBox`](#ballotbox).
4.  **Ballot Processing & Tallying**:
    - Searches the `ballot_tallies` array for an existing entry matching the `weather_status`.
    - If found, adds the operator's `stake_weights` to the `stake_weights` field of the existing [`BallotTally`](#ballottally) and increments the raw `tally` counter.
    - If not found, initializes a new `BallotTally` entry with the `weather_status`, the operator's `stake_weights`, and a `tally` of 1. Increments `unique_ballots`.
5.  **Consensus Calculation & Result Creation**:
    - Retrieves the total `stake_weights` from the `EpochSnapshot`.
    - Compares the winning ballot's accumulated `stake_weights` against the total.
    - If the threshold is met *and* consensus hasn't already been marked:
        - Sets the `winning_ballot` field in the `BallotBox`.
        - Records the current `slot` in `slot_consensus_reached`.
        - Updates the `EpochState`.
        - Invokes an instruction (likely via CPI or separate transaction) to create the `ConsensusResult` account, storing the winning status, epoch, weights, and slot.
6.  **Cross-Validation**: Implicitly ensures the vote aligns with the correct `ncn` and `epoch` through the PDAs used for the involved accounts (`BallotBox`, `OperatorSnapshot`, `EpochState`).

This multi-layered architecture ensures votes are processed securely, tallied correctly using the snapshotted weights, and that consensus is determined accurately based on stake-weighted participation.

#### 7.5 Security Considerations in the Voting Process

The voting process incorporates several key security features:

1.  **Sybil Attack Prevention**:
    - Voting power is derived directly from stake weight captured in immutable [`OperatorSnapshot`](#operatorsnapshot) accounts, not operator count.
    - Operators with zero snapshotted stake weight cannot vote, preventing attacks based on creating numerous fake operators.
2.  **Replay Protection**:
    - The [`BallotBox`](#ballotbox) tracks which operators have voted (`operator_votes` array).
    - Attempts by an operator to vote more than once within the same epoch are rejected.
3.  **Time-Bound Voting**:
    - Votes are only accepted if the [`EpochState`](#epochaccountstatus) indicates the voting phase is active for the specified `epoch`.
    - While votes might be accepted slightly after consensus is reached (within `valid_slots_after_consensus`), they won't change the already determined outcome.
4.  **Authority Verification**:
    - The `cast_vote` instruction requires a signature from the `operator_admin`, ensuring only the authorized entity can cast a vote for that operator.
5.  **Tamper-Proof Tallying**:
    - Votes are tallied based on immutable [`OperatorSnapshot`](#operatorsnapshot) data created *before* voting began.
    - The system prevents retroactive changes to stake weights from affecting an ongoing or completed vote tally for that epoch.
6.  **Consistent Threshold**:
    - The consensus threshold is calculated based on the total stake weight recorded in the [`EpochSnapshot`](#epochsnapshot), providing a fixed target for the epoch.

These security measures ensure the voting process remains resilient against various attack vectors and manipulation attempts, maintaining the integrity of the consensus mechanism.

### 8. Verification

The Verification phase validates that the voting process completed successfully and that the expected consensus was achieved. This critical step confirms the integrity of the entire system by examining the on-chain data structures ([`BallotBox`](#ballotbox) and [`ConsensusResult`](#consensusresult)) and verifying they contain the expected results.

#### 8.1 Ballot Box Verification

```rust
// 6. Verify voting results
let ballot_box = ncn_program_client.get_ballot_box(ncn_pubkey, epoch).await?;
assert!(ballot_box.has_winning_ballot());
assert!(ballot_box.is_consensus_reached());
assert_eq!(
    ballot_box.get_winning_ballot().unwrap().weather_status(),
    winning_weather_status
);
```

The first verification step examines the `BallotBox` account for the completed epoch:

1.  **Winning Ballot Check**:
    - `has_winning_ballot()` confirms that the `winning_ballot` field within the `BallotBox` structure is marked as valid. This happens only when a ballot option crosses the consensus threshold.
2.  **Consensus Status Check**:
    - `is_consensus_reached()` checks if the `slot_consensus_reached` field is greater than zero, indicating the consensus condition was met during the voting process.
3.  **Outcome Verification**:
    - The test retrieves the `winning_ballot` struct and asserts that its `weather_status` field matches the `winning_weather_status` defined earlier (`WeatherStatus::Sunny`). This confirms the correct outcome was identified based on the stake-weighted tally.

Verifying the `BallotBox` ensures the core voting and tallying mechanism functioned correctly during the active epoch.

#### 8.2 Consensus Result Account Verification

```rust
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

The second verification step examines the `ConsensusResult` account, which serves as the permanent, immutable record of the voting outcome:

1.  **Consensus Result Existence & Fetching**:
    - The test successfully fetches the `ConsensusResult` account using its PDA derived from the NCN pubkey and epoch. Its existence implies consensus was reached and the account was created.
2.  **Consensus Status Validation**:
    - `is_consensus_reached()` checks an internal flag derived from stored values (like `consensus_slot` > 0), confirming the outcome is officially recognized.
3.  **Metadata Verification**:
    - Asserts that the `epoch` field matches the current epoch.
    - Asserts that the `weather_status` matches the expected `winning_weather_status`.
4.  **Cross-Account Consistency Check**:
    - Fetches the `BallotBox` again.
    - Retrieves the `BallotTally` corresponding to the winning ballot from the `BallotBox`.
    - Asserts that the `vote_weight` stored in the `ConsensusResult` exactly matches the `stake_weight` recorded in the winning `BallotTally` within the `BallotBox`. This ensures consistency between the temporary voting record and the permanent result.
5.  **Detailed Reporting**:
    - Prints key details from the verified `ConsensusResult` account for confirmation. Note: The `consensus_recorder()` field mentioned in the print statement isn't present in the provided struct definition for `ConsensusResult`; this might be a documentation inconsistency or relate to a field not shown.

Verifying the `ConsensusResult` confirms that the outcome was durably stored with the correct details and consistent with the voting process itself.

#### 8.3 Architecture of Verification and Result Persistence

The verification phase highlights several important architectural features:

1.  **Dual Record Keeping**:
    - The system temporarily uses the `BallotBox` during the epoch for active voting and tallying.
    - Upon reaching consensus, it creates a separate, permanent `ConsensusResult` account.
    - This redundancy allows for cleanup while preserving the essential outcome.
2.  **Separation of Process and Outcome**:
    - The `BallotBox` (process) can eventually be closed to reclaim rent.
    - The `ConsensusResult` (outcome) persists indefinitely as the historical record.
3.  **Automated Result Creation**:
    - The `ConsensusResult` account is typically created automatically within the `cast_vote` instruction (or a closely related one) when the consensus threshold is first met. This ensures timely recording without requiring a separate administrative action.
4.  **Result Immutability**:
    - The `ConsensusResult` account, once created, is designed to be immutable. It stores the outcome based on the state when consensus was reached.
5.  **Time and Slot Tracking**:
    - Both `BallotBox` and `ConsensusResult` store key timing information (`slot_consensus_reached`, `epoch`). This metadata is crucial for auditing and understanding the system's behavior over time.

#### 8.4 Verification Techniques and Best Practices

The verification approach demonstrates several best practices:

1.  **Multi-Level Verification**: Testing both the ephemeral process account (`BallotBox`) and the persistent outcome account (`ConsensusResult`) provides comprehensive validation.
2.  **State Assertions**: Using dedicated helper functions on the deserialized accounts (`has_winning_ballot()`, `is_consensus_reached()`) makes tests more readable and robust against internal representation changes.
3.  **Equality Assertions**: Using strict equality (`assert_eq!`) for key outcome data (winning status, epoch, weights) ensures exactness.
4.  **Cross-Structure Validation**: Comparing critical values (like `vote_weight`) between the `BallotBox` and `ConsensusResult` confirms data consistency across different parts of the system.
5.  **Complete Outcome Validation**: Checking not just the winning choice but also associated metadata (epoch, weights, consensus flags) catches more subtle errors.
6.  **Clear Reporting**: Outputting verified data (`println!`) provides immediate feedback during test runs.

This rigorous verification ensures the NCN system reliably achieves and records stake-weighted consensus according to its design.

### 9. Cleanup

After the core functionality has been tested and verified for a given epoch, the temporary accounts associated with that epoch can be closed to reclaim the SOL locked for rent. The persistent `ConsensusResult` account remains.

```rust
// 8. Close epoch accounts but keep consensus result
let epoch_before_closing_account = fixture.clock().await.epoch;
fixture.close_epoch_accounts_for_test_ncn(&test_ncn).await?;

// Verify that consensus_result account is not closed
{
    let consensus_result = ncn_program_client
        .get_consensus_result(ncn_pubkey, epoch_before_closing_account)
        .await?;

    // Verify consensus_result account exists and has correct values
    assert!(consensus_result.is_consensus_reached());
    assert_eq!(consensus_result.epoch(), epoch_before_closing_account);
}
```

This cleanup process involves:

1.  **Identifying Epoch**: Recording the current epoch (`epoch_before_closing_account`) just before initiating closure.
2.  **Closing Accounts**: Calling `fixture.close_epoch_accounts_for_test_ncn`, which likely iterates through epoch-specific accounts (`EpochState`, `WeightTable`, `EpochSnapshot`, all `OperatorSnapshot`s, `BallotBox`, potentially `VaultOperatorDelegationSnapshot`s) and invokes a `close_epoch_account` instruction (or similar) for each. This instruction typically transfers the rent SOL back to a designated recipient (often the original payer or a treasury) and closes the account. An `EpochMarker` account might be created to signify successful cleanup.
3.  **Verifying Persistence**: After the cleanup function returns, the test attempts to fetch the `ConsensusResult` account for the *same epoch* again.
4.  **Confirming Data**: It asserts that the fetched `ConsensusResult` still exists and retains its key data (`is_consensus_reached`, `epoch`), confirming it was *not* closed during the cleanup process.

This demonstrates a crucial design feature:

- **Resource Management**: Temporary accounts used only during the voting cycle are cleaned up, preventing indefinite accumulation of rent-paying accounts.
- **Outcome Preservation**: The final, critical outcome (`ConsensusResult`) is preserved as a permanent on-chain record, suitable for historical lookups or use by other programs.

This efficient cleanup mechanism allows the NCN system to operate continuously over many epochs without unbounded growth in account storage requirements.

## Core Struct Definitions

Here are the core data structures defined in the `/core/src` directory, used throughout the NCN program:

### Config

file: `config.rs`

- **Purpose**: Stores global, long-lived configuration parameters for the NCN program instance.
- **Definition**:
  ```rust
  #[derive(Debug, Clone, Copy, Zeroable, Pod, AccountDeserialize, ShankAccount)]
  #[repr(C)]
  pub struct Config {
      /// The Restaking program's NCN admin is the signer to create and update this account
      pub ncn: Pubkey,
      /// The admin to update the tie breaker - who can decide the meta merkle root when consensus is reached
      pub tie_breaker_admin: Pubkey,
      /// Number of slots after consensus reached where voting is still valid
      pub valid_slots_after_consensus: PodU64,
      /// Number of epochs before voting is considered stalled
      pub epochs_before_stall: PodU64,
      /// Number of epochs after consensus reached where voting is still valid
      pub epochs_after_consensus_before_close: PodU64,
      /// Only epochs after this epoch are valid for voting
      pub starting_valid_epoch: PodU64,
      /// Bump seed for the PDA
      pub bump: u8,
  }
  ```
- **Explanation**: Holds the associated `ncn`, the `tie_breaker_admin`, and various timing/threshold parameters (`valid_slots_after_consensus`, `epochs_before_stall`, `epochs_after_consensus_before_close`, `starting_valid_epoch`).

### Ballot

file: `ballot_box.rs`

- **Purpose**: Represents a single potential outcome in the consensus process, specifically a weather status in this example.
- **Definition**:
  ```rust
  #[derive(Debug, Clone, Copy, Zeroable, ShankType, Pod)]
  #[repr(C)]
  pub struct Ballot {
      /// The weather status value
      weather_status: u8,
      /// Whether the ballot is valid
      is_valid: PodBool,
  }
  ```
- **Explanation**: Holds the numeric `weather_status` being voted on and a boolean `is_valid` flag to ensure it corresponds to a known status.

### BallotTally

file: `ballot_box.rs`

- **Purpose**: Aggregates votes and stake weight for a specific `Ballot`.
- **Definition**:
  ```rust
  #[derive(Debug, Clone, Copy, Zeroable, ShankType, Pod)]
  #[repr(C)]
  pub struct BallotTally {
      /// Index of the tally within the ballot_tallies
      index: PodU16,
      /// The ballot being tallied
      ballot: Ballot,
      /// Breakdown of all of the stake weights that contribute to the vote
      stake_weights: StakeWeights,
      /// The number of votes for this ballot
      tally: PodU64,
  }
  ```
- **Explanation**: Tracks which `ballot` this tally is for, its `index` in the main array, the total `stake_weights` supporting it, and the raw `tally` (count) of votes.

### OperatorVote

file: `ballot_box.rs`

- **Purpose**: Records the vote cast by a single operator in a specific epoch.
- **Definition**:
  ```rust
  #[derive(Debug, Clone, Copy, Zeroable, ShankType, Pod)]
  #[repr(C)]
  pub struct OperatorVote {
      /// The operator that cast the vote
      operator: Pubkey,
      /// The slot when the operator voted
      slot_voted: PodU64,
      /// The stake weights of the operator
      stake_weights: StakeWeights,
      /// The index of the ballot in the ballot_tallies
      ballot_index: PodU16,
  }
  ```
- **Explanation**: Stores the `operator` pubkey, the `slot_voted`, their `stake_weights` at that time, and the `ballot_index` they voted for.

### BallotBox

file: `ballot_box.rs`

- **Purpose**: The central account for managing the voting process within a specific epoch. It collects votes, tallies them, and determines consensus.
- **Definition**:
  ```rust
  #[derive(Debug, Clone, Copy, Zeroable, Pod, AccountDeserialize, ShankAccount)]
  #[repr(C)]
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
      operator_votes: [OperatorVote; MAX_OPERATORS],
      /// Mapping of ballots votes to stake weight
      ballot_tallies: [BallotTally; MAX_OPERATORS],
  }
  ```
- **Explanation**: Holds metadata (`ncn`, `epoch`, timestamps), vote counts (`operators_voted`, `unique_ballots`), the `winning_ballot` (if consensus reached), and arrays for individual `operator_votes` and aggregated `ballot_tallies`.

### ConsensusResult

file: `consensus_result.rs`

- **Purpose**: A persistent account storing the final outcome of a consensus cycle for a specific epoch. It remains even after epoch-specific accounts are closed.
- **Definition**:
  ```rust
  #[derive(Debug, Clone, Copy, Zeroable, Pod, AccountDeserialize, ShankAccount)]
  #[repr(C)]
  pub struct ConsensusResult {
      /// The NCN this consensus result is for
      ncn: Pubkey,
      /// The epoch this consensus result is for
      epoch: PodU64,
      /// The vote weight that supported the winning status
      vote_weight: PodU64,
      /// The total vote weight in the ballot box
      total_vote_weight: PodU64,
      /// The slot at which consensus was reached
      consensus_slot: PodU64,
      /// Bump seed for the PDA
      bump: u8,
      /// The winning weather status that reached consensus
      weather_status: u8,
  }
  ```
- **Explanation**: Stores the `ncn`, `epoch`, the winning `weather_status`, the `vote_weight` supporting it, the `total_vote_weight` possible in that epoch, and the `consensus_slot`.

### AccountPayer

file: `account_payer.rs`

- **Purpose**: An empty, uninitialized system account used solely as a Program Derived Address (PDA) to hold SOL temporarily for paying rent during account creation or reallocation within the NCN program.
- **Definition**:
  ```rust
  pub struct AccountPayer {}
  ```
- **Explanation**: This is a marker struct with no fields. Its associated functions handle deriving the PDA and performing SOL transfers for rent payments using `invoke_signed`.

### EpochMarker

file: `epoch_marker.rs`

- **Purpose**: An empty account created as a marker to signify that all temporary accounts associated with a specific NCN epoch have been successfully closed.
- **Definition**:
  ```rust
  #[derive(Debug, Clone, Copy, Zeroable, ShankType, Pod, AccountDeserialize, ShankAccount)]
  #[repr(C)]
  pub struct EpochMarker {
      ncn: Pubkey,
      epoch: PodU64,
      slot_closed: PodU64,
  }
  ```
- **Explanation**: Contains the `ncn`, the `epoch` that was closed, and the `slot_closed`. Its existence confirms cleanup completion for that epoch.

### EpochSnapshot

file: `epoch_snapshot.rs`

- **Purpose**: Captures the overall state of the NCN system at the beginning of a specific epoch, including participant counts and total potential stake weight.
- **Definition**:
  ```rust
  #[derive(Debug, Clone, Copy, Zeroable, Pod, AccountDeserialize, ShankAccount)]
  #[repr(C)]
  pub struct EpochSnapshot {
      /// The NCN this snapshot is for
      ncn: Pubkey,
      /// The epoch this snapshot is for
      epoch: PodU64,
      /// Bump seed for the PDA
      bump: u8,
      /// Slot Epoch snapshot was created
      slot_created: PodU64,
      /// Slot Epoch snapshot was finalized
      slot_finalized: PodU64,
      /// Number of operators in the epoch
      operator_count: PodU64,
      /// Number of vaults in the epoch
      vault_count: PodU64,
      /// Keeps track of the number of completed operator registration through `snapshot_vault_operator_delegation` and `initialize_operator_snapshot`
      operators_registered: PodU64,
      /// Keeps track of the number of valid operator vault delegations
      valid_operator_vault_delegations: PodU64,
      /// Tallies the total stake weights for all vault operator delegations
      stake_weights: StakeWeights,
  }
  ```
- **Explanation**: Stores metadata (`ncn`, `epoch`, timestamps), counts (`operator_count`, `vault_count`), progress trackers (`operators_registered`, `valid_operator_vault_delegations`), and the total aggregated `stake_weights` for the epoch.

### OperatorSnapshot

file: `epoch_snapshot.rs`

- **Purpose**: Captures the state of a single operator, including their total delegated stake and its breakdown by vault/token type, at the beginning of a specific epoch.
- **Definition**:
  ```rust
  #[derive(Debug, Clone, Copy, Zeroable, Pod, AccountDeserialize, ShankAccount)]
  #[repr(C)]
  pub struct OperatorSnapshot {
      operator: Pubkey,
      ncn: Pubkey,
      ncn_epoch: PodU64,
      bump: u8,
      slot_created: PodU64,
      slot_finalized: PodU64,
      is_active: PodBool,
      ncn_operator_index: PodU64,
      operator_index: PodU64,
      operator_fee_bps: PodU16,
      vault_operator_delegation_count: PodU64,
      vault_operator_delegations_registered: PodU64,
      valid_operator_vault_delegations: PodU64,
      stake_weights: StakeWeights,
      vault_operator_stake_weight: [VaultOperatorStakeWeight; MAX_VAULTS],
  }
  ```
- **Explanation**: Contains operator/NCN identifiers, timestamps, status (`is_active`), indices, `operator_fee_bps`, delegation counts/progress, the operator's total `stake_weights`, and a detailed breakdown in `vault_operator_stake_weight`.

### VaultOperatorStakeWeight

file: `epoch_snapshot.rs`

- **Purpose**: A helper struct within `OperatorSnapshot` to store the calculated stake weight originating from a specific vault's delegation to that operator.
- **Definition**:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Zeroable, Pod)]
  pub struct VaultOperatorStakeWeight {
      vault: Pubkey,
      vault_index: PodU64,
      stake_weight: StakeWeights,
  }
  ```
- **Explanation**: Links a `vault` pubkey and `vault_index` to the specific `stake_weight` derived from its delegation to the parent `OperatorSnapshot`.

### StMintEntry

file: `vault_registry.rs`

- **Purpose**: Represents a supported token mint within the `VaultRegistry`, storing its address and associated voting weight.
- **Definition**:
  ```rust
  #[derive(Debug, Clone, Copy, Zeroable, ShankType, Pod)]
  #[repr(C)]
  pub struct StMintEntry {
      /// The supported token ( ST ) mint
      st_mint: Pubkey,
      // Either a switchboard feed or a weight must be set
      /// The switchboard feed for the mint
      reserve_switchboard_feed: [u8; 32],
      /// The weight
      weight: PodU128,
  }
  ```
- **Explanation**: Stores the `st_mint` address and its assigned voting `weight`. `reserve_switchboard_feed` is unused here.

### VaultEntry

file: `vault_registry.rs`

- **Purpose**: Represents a registered vault within the `VaultRegistry`.
- **Definition**:
  ```rust
  #[derive(Debug, Clone, Copy, Zeroable, ShankType, Pod)]
  #[repr(C)]
  pub struct VaultEntry {
      /// The vault account
      vault: Pubkey,
      /// The supported token ( ST ) mint of the vault
      st_mint: Pubkey,
      /// The index of the vault in respect to the NCN account
      vault_index: PodU64,
      /// The slot the vault was registered
      slot_registered: PodU64,
  }
  ```
- **Explanation**: Stores the `vault` address, the `st_mint` it holds, its assigned `vault_index`, and the `slot_registered`.

### VaultRegistry

file: `vault_registry.rs`

- **Purpose**: A global account for the NCN program instance that maintains the list of all supported token mints and all registered vaults participating in the system.
- **Definition**:
  ```rust
  #[derive(Debug, Clone, Copy, Zeroable, Pod, AccountDeserialize, ShankAccount)]
  #[repr(C)]
  pub struct VaultRegistry {
      /// The NCN the vault registry is associated with
      pub ncn: Pubkey,
      /// The bump seed for the PDA
      pub bump: u8,
      /// The list of supported token ( ST ) mints
      pub st_mint_list: [StMintEntry; MAX_ST_MINTS],
      /// The list of vaults
      pub vault_list: [VaultEntry; MAX_VAULTS],
  }
  ```
- **Explanation**: Holds the `ncn` identifier, `bump`, and arrays for `st_mint_list` (supported tokens and weights) and `vault_list` (registered vaults).

### WeightTable

file: `weight_table.rs`

- **Purpose**: An epoch-specific account that snapshots the weights of all supported tokens at the beginning of the epoch.
- **Definition**:
  ```rust
  #[derive(Debug, Clone, Copy, Zeroable, Pod, AccountDeserialize, ShankAccount)]
  #[repr(C)]
  pub struct WeightTable {
      /// The NCN the account is associated with
      ncn: Pubkey,
      /// The epoch the account is associated with
      epoch: PodU64,
      /// Slot weight table was created
      slot_created: PodU64,
      /// Number of vaults in tracked mints at the time of creation
      vault_count: PodU64,
      /// Bump seed for the PDA
      bump: u8,
      /// A snapshot of the Vault Registry
      vault_registry: [VaultEntry; MAX_VAULTS],
      /// The weight table
      table: [WeightEntry; MAX_ST_MINTS],
  }
  ```
- **Explanation**: Contains metadata (`ncn`, `epoch`, `slot_created`, `vault_count`), a snapshot of the `vault_registry` at creation, and the main `table` holding `WeightEntry` structs with the frozen weights for the epoch.

### EpochAccountStatus

file: `epoch_state.rs`

- **Purpose**: A helper struct within `EpochState` used to track the lifecycle status (e.g., DNE, Created, Closed) of various accounts associated with a specific epoch.
- **Definition**:
  ```rust
  #[derive(Debug, Clone, Copy, Zeroable, ShankType, Pod)]
  #[repr(C)]
  pub struct EpochAccountStatus {
      epoch_state: u8,
      weight_table: u8,
      epoch_snapshot: u8,
      operator_snapshot: [u8; MAX_OPERATORS],
      ballot_box: u8,
  }
  ```
- **Explanation**: Uses `u8` fields to represent the status (`