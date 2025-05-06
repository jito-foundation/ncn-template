# Simulation Test Detailed Guide

## Table of Contents

1. [Overview](#overview)
1. [Prerequisites](#prerequisites)
1. [Test Flow](#test-flow)
   1. [Environment Setup](#1-environment-setup)
   1. [NCN Setup](#2-ncn-setup)
   1. [Operators and Vaults Setup](#3-operators-and-vaults-setup)
      1. [Operator Creation and NCN Connection](#31-operator-creation-and-ncn-connection)
      2. [Vault Creation for Different Token Types](#32-vault-creation-for-different-token-types)
      3. [Delegation Setup](#33-delegation-setup)
      4. [Delegation Architecture and Voting Power Calculation](#34-delegation-architecture-and-voting-power-calculation)
   1. [NCN Program Configuration](#4-ncn-program-configuration)
      1. [Program Configuration Initialization](#41-program-configuration-initialization)
      2. [Vault Registry Initialization](#42-vault-registry-initialization)
      3. [Activating Relationships with Time Advancement](#43-activating-relationships-with-time-advancement)
      4. [Token Registration and Weight Assignment](#44-token-registration-and-weight-assignment)
      5. [Vault Registration](#45-vault-registration)
      6. [Architecture Considerations](#46-architecture-considerations)
   1. [Epoch Snapshot and Voting Preparation](#5-epoch-snapshot-and-voting-preparation)
      1. [Epoch State Initialization](#51-epoch-state-initialization)
      2. [Weight Table Initialization and Population](#52-weight-table-initialization-and-population)
      3. [Epoch Snapshot Creation](#53-epoch-snapshot-creation)
      4. [Operator Snapshots](#54-operator-snapshots)
      5. [Vault-Operator Delegation Snapshots](#55-vault-operator-delegation-snapshots)
      6. [Ballot Box Initialization](#56-ballot-box-initialization)
      7. [Architecture and Security Considerations](#57-architecture-and-security-considerations)
   1. [Voting Process](#6-voting-process)
      1. [Setting the Expected Outcome](#61-setting-the-expected-outcome)
      2. [Testing Zero-Delegation Operator Restrictions](#62-testing-zero-delegation-operator-restrictions)
      3. [Distributing Votes Across Different Options](#63-distributing-votes-across-different-options)
      4. [Establishing Consensus Through Majority Voting](#64-establishing-consensus-through-majority-voting)
      5. [Vote Processing Architecture](#65-vote-processing-architecture)
      6. [Security Considerations in the Voting Process](#66-security-considerations-in-the-voting-process)
   1. [Verification](#7-verification)
      1. [Ballot Box Verification](#71-ballot-box-verification)
      2. [Consensus Result Account Verification](#72-consensus-result-account-verification)
      3. [Architecture of Verification and Result Persistence](#73-architecture-of-verification-and-result-persistence)
      4. [Verification Techniques and Best Practices](#74-verification-techniques-and-best-practices)
   1. [Cleanup](#8-cleanup)
1. [Detailed Function Explanations](#detailed-function-explanations)
   1. [Core Setup Functions](#core-setup-functions)
   1. [NCN Setup Functions](#ncn-setup-functions)
   1. [Operator and Vault Setup Functions](#operator-and-vault-setup-functions)
   1. [NCN Program Configuration Functions](#ncn-program-configuration-functions)
   1. [Epoch Snapshot and Voting Preparation Functions](#epoch-snapshot-and-voting-preparation-functions)
   1. [Voting and Verification Functions](#voting-and-verification-functions)
   1. [WeatherStatus Enum](#weatherstatus-enum)
1. [Expected Outcomes](#expected-outcomes)
1. [Error Cases](#error-cases)
1. [Fuzz Testing](#fuzz-testing)

## Overview

The simulation test is a comprehensive test case that simulates a complete NCN (Node Consensus Network) system with multiple operators, vaults, and token types. It tests the entire flow from setup to voting and consensus reaching. The system uses Jito's restaking infrastructure and custom voting logic to coordinate network participants.

## Prerequisites

Before running the simulation test, ensure you have:

1. Set up the test ledger using `./ncn-program-operator-cli/scripts/setup-test-ledger.sh`
1. Built the NCN program using `cargo build-sbf`
1. Set the correct Solana version (1.18.26 recommended)

This setup:


## Test Flow

### 1. Environment Setup

The test begins with initializing the test environment:

```rust
let mut fixture = TestBuilder::new().await;
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
```

This code:
1. Initializes the test environment and required program clients
2. Configures 13 operators
3. Sets up 4 different token types with different weights for voting power
4. Defines delegation amounts ranging from minimal (1 lamport) to very large (10k tokens)

### 2. NCN Setup

```rust
// 2.a. Initialize the test NCN account using the Restaking program By Jito
let mut test_ncn = fixture.create_test_ncn().await?;
let ncn_pubkey = test_ncn.ncn_root.ncn_pubkey;
```

This step:
- Creates a new Node Consensus Network (NCN) using Jito's restaking infrastructure
- Stores the NCN public key for future operations

### 3. Operators and Vaults Setup

The Operators and Vaults setup phase is critical to the simulation as it establishes the network of participants and their relationships. This creates the foundation for the consensus and voting mechanisms being tested.

#### 3.1 Operator Creation and NCN Connection

```rust
// 2.b. Initialize operators and associate with NCN
fixture
    .add_operators_to_test_ncn(&mut test_ncn, OPERATOR_COUNT, Some(100))
    .await?;
```

This step:
- Creates 13 operator accounts using Jito's restaking program
- Sets each operator's fee to 100 basis points (1%)
- Establishes a bidirectional handshake between each operator and the NCN

The handshake process involves:
1. Creating operator accounts with their respective admin keypairs
2. Initializing the NCN-operator relationship state using `do_initialize_ncn_operator_state`
3. Warming up the NCN-to-operator connection using `do_ncn_warmup_operator`
4. Warming up the operator-to-NCN connection using `do_operator_warmup_ncn`

These bidirectional relationships are essential for the security model, ensuring operators can only participate in voting if they have a valid, active connection to the NCN.

#### 3.2 Vault Creation for Different Token Types

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

This step creates a total of 7 vaults distributed across 4 different token types:
- 3 vaults for TKN1 (base weight)
- 2 vaults for TKN2 (double weight)
- 1 vault for TKN3 (triple weight)
- 1 vault for TKN4 (quadruple weight)

For each vault, the system:
1. Initializes a vault account via the vault program with zero fees (important for testing)
2. Creates a vault mint (token) if not provided directly
3. Establishes a bidirectional handshake between the vault and the NCN:
   - Initializes an NCN-vault ticket using `do_initialize_ncn_vault_ticket`
   - Warms up the ticket using `do_warmup_ncn_vault_ticket`
   - Creates a vault-NCN ticket using `do_initialize_vault_ncn_ticket`
   - Advances slots to ensure the relationship activates

The distribution of vaults across different token types enables testing how the system handles voting power with different token weights and concentrations.

#### 3.3 Delegation Setup

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
- Every vault delegates to every operator (except the last one)
- Delegation amounts cycle through the `delegations` array (which ranges from 1 lamport to 10,000 tokens)
- The last operator intentionally receives zero delegation to test the system's handling of operators without stake
- The delegation is performed directly through the vault program using `do_add_delegation`

#### 3.4 Delegation Architecture and Voting Power Calculation

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

### 4. NCN Program Configuration

The NCN Program Configuration phase establishes the on-chain infrastructure necessary for the voting and consensus mechanisms. This includes setting up configuration parameters, creating data structures, and registering the token types and vaults that will participate in the system.

#### 4.1 Program Configuration Initialization

```rust
// 3.a. Initialize the config for the ncn-program
ncn_program_client
    .do_initialize_config(test_ncn.ncn_root.ncn_pubkey, &test_ncn.ncn_root.ncn_admin)
    .await?;
```

This step initializes the core configuration for the NCN program with critical parameters:
- **NCN Admin**: The authority that can modify configuration settings
- **Epochs Before Stall**: How many epochs before a non-completed voting cycle is considered stalled (default: 3)
- **Epochs After Consensus Before Close**: How long to wait after consensus before closing epoch data (default: 10)
- **Valid Slots After Consensus**: How many slots votes are still accepted after consensus is reached (default: 10000)

Under the hood, this creates a `NcnConfig` account that stores these parameters and serves as the authoritative configuration for this NCN instance.

#### 4.2 Vault Registry Initialization

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

The registry creates a `VaultRegistry` account that stores this information on-chain for the NCN program to access during voting operations.

#### 4.3 Activating Relationships with Time Advancement

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

#### 4.4 Token Registration and Weight Assignment

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
- The NCN admin can update the weights of the tokens at any time, which will affect the voting power of the delegations in the next voting cycle

The weight assignment is fundamental to the design, allowing different tokens to have varying influence on the voting process based on their economic significance or other criteria determined by the NCN administrators.

#### 4.5 Vault Registration

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

#### 4.6 Architecture Considerations

The NCN program configuration establishes a multi-layered security model:
1. **Authentication Layer**: Only the NCN admin can initialize configuration and register tokens
2. **Relationship Layer**: Only vaults and operators with established, active handshakes can participate
3. **Time Security Layer**: Enforced waiting periods prevent quick creation and use of malicious actors
4. **Registry Layer**: All participants must be registered and tracked in on-chain registries

This layered approach ensures the integrity of the voting system by validating the identity and relationships of all participants before they can influence the consensus process.

The configuration phase completes the preparation of the system's infrastructure, setting the stage for the actual voting mechanics to begin in subsequent phases.

### 5. Epoch Snapshot and Voting Preparation

The Epoch Snapshot and Voting Preparation phase is where the system captures the current state of all participants and prepares the infrastructure for voting. This is an essential component of the architecture as it ensures voting is based on a consistent, verifiable snapshot of the network state at a specific moment in time.

#### 5.1 Epoch State Initialization

```rust
// 4.a. Initialize the epoch state for the current epoch
fixture.add_epoch_state_for_test_ncn(&test_ncn).await?;
```

The epoch state serves as the control center for the current voting cycle:
- It creates an `EpochState` account tied to the specific NCN and epoch
- This account tracks the progress through each stage of the voting cycle
- It maintains flags for each phase (weight setting, snapshot taking, voting, closing)
- The epoch state provides protection against out-of-sequence operations
- It stores metadata like the current epoch, slot information, and participant counts

Once initialized, the epoch state becomes the authoritative record of where the system is in the voting process, preventing operations from happening out of order or in duplicate.

#### 5.2 Weight Table Initialization and Population

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

The weight table mechanism handles the token weights for the current epoch:

1. **Weight Table Initialization**:
   - Creates a `WeightTable` account for the specific epoch
   - Allocates space based on the number of supported tokens
   - Links the table to the NCN and current epoch
   - Initializes the table structure with empty entries

2. **Weight Setting**:
   - Copies the current weights from the vault registry to the weight table
   - "Freezes" these weights for the duration of the voting cycle
   - Updates the epoch state to mark weight setting as complete
   - Creates an immutable record of token weights that will be used for voting

This two-step process is critical for the integrity of the system as it:
- Creates a permanent record of weights at the time voting begins
- Prevents weight changes during a voting cycle from affecting ongoing votes
- Allows transparent verification of the weights used for a particular vote
- Enables historical auditing of how weights changed over time

#### 5.3 Epoch Snapshot Creation

```rust
// 4.d. Take the epoch snapshot
fixture.add_epoch_snapshot_to_test_ncn(&test_ncn).await?;
```

The epoch snapshot captures the aggregate state of the entire system:
- Creates an `EpochSnapshot` account for the NCN and epoch
- Records the total number of operators and vaults
- Captures the total stake weight across all participants
- Stores important metadata such as the snapshot creation slot
- Serves as the reference point for total voting power calculations

This global snapshot provides the denominator for consensus calculations - the total possible voting power in the system - which is essential for determining when consensus (e.g., 66% of total stake) has been reached.

#### 5.4 Operator Snapshots

```rust
// 4.e. Take snapshots for all operators
fixture
    .add_operator_snapshots_to_test_ncn(&test_ncn)
    .await?;
```

For each operator in the system:
- Creates an `OperatorSnapshot` account linked to the operator, NCN, and epoch
- Records the operator's total delegated stake at this moment
- Captures the stake weight breakdown across different token types
- Verifies the operator has active handshakes with the NCN
- Validates the operator's eligibility to participate in voting

These snapshots establish each operator's voting power for the current epoch, ensuring that later delegations or withdrawals cannot alter the voting weight once the snapshot is taken. This prevents manipulation of the voting process through last-minute stake changes.

#### 5.5 Vault-Operator Delegation Snapshots

```rust
// 4.f. Record all vault-to-operator delegations
fixture
    .add_vault_operator_delegation_snapshots_to_test_ncn(&test_ncn)
    .await?;
```

For each active vault-to-operator delegation:
- Creates a `VaultOperatorDelegationSnapshot` account
- Records the exact delegation amount at the current moment
- Links the snapshot to the specific vault, operator, NCN, and epoch
- Multiplies the delegation by the corresponding token weight
- Adds this weighted delegation to the operator's total stake weight

These granular snapshots serve multiple purposes:
- They provide detailed audit trails of exactly where each operator's voting power comes from
- They enable verification of correct weight calculation for each delegation
- They prevent retroactive manipulation of the voting power distribution
- They allow historical analysis of delegation patterns and their impact on voting

#### 5.6 Ballot Box Initialization

```rust
// 4.g. Initialize the ballot box for collecting votes
fixture.add_ballot_box_to_test_ncn(&test_ncn).await?;
```

The final preparation step creates the ballot box:
- Initializes a `BallotBox` account linked to the NCN and epoch
- Creates arrays to track operator votes and ballot tallies
- Sets up the data structures for recording and counting votes
- Prepares the consensus tracking mechanism
- Links the ballot box to the epoch state for progress tracking

The ballot box becomes the central repository where all votes are recorded and tallied during the voting process. It is designed to efficiently track:
- Which operators have voted and what they voted for
- The cumulative stake weight behind each voting option
- The current winning ballot (if any)
- Whether consensus has been reached

#### 5.7 Architecture and Security Considerations

The snapshot system implements several key architectural principles:

1. **Point-in-Time Consistency**: All snapshots capture the system state at approximately the same moment, creating a consistent view.

2. **Immutability**: Once taken, snapshots cannot be modified, ensuring the integrity of the voting process.

3. **Layered Verification**: The system enables verification at multiple levels:
   - Aggregate level (epoch snapshot)
   - Participant level (operator snapshots)
   - Relationship level (delegation snapshots)

4. **Defense Against Time-Based Attacks**: By freezing the state before voting begins, the system prevents:
   - Late stake additions to influence outcomes
   - Strategic withdrawals after seeing early votes
   - Any form of "stake voting power front-running"

5. **Separation of State and Process**: 
   - The state (snapshots) is captured separately from the process (voting)
   - This clear separation simplifies reasoning about the system
   - It enables more effective testing and verification

The comprehensive snapshot approach ensures that voting occurs on a well-defined, verifiable view of the network's state, establishing a solid foundation for the actual voting process to follow.

### 6. Voting Process

The Voting Process is the core functionality of the NCN system, where operators express their preferences on the network state (represented by the "weather status" in this simulation). This process leverages the infrastructure and snapshots created in previous steps to ensure secure, verifiable, and stake-weighted consensus.

#### 6.1 Setting the Expected Outcome

```rust
// Define the expected winning weather status
let winning_weather_status = WeatherStatus::Sunny as u8;
```

For testing purposes, the system defines an expected outcome. In a production environment, this would be determined organically through actual operator votes. The weather status enum (`Sunny`, `Cloudy`, `Rainy`) serves as a simplified proxy for any on-chain decision that requires consensus.

#### 6.2 Testing Zero-Delegation Operator Restrictions

```rust
// 5. Cast votes from operators
{
    let epoch = fixture.clock().await.epoch;

    let zero_delegation_operator = test_ncn.operators.last().unwrap(); // Operator with no delegations
    let first_operator = &test_ncn.operators[0];
    let second_operator = &test_ncn.operators[1];
    let third_operator = &test_ncn.operators[2];

    // Attempt vote from zero_delegation_operator (should fail)
    {
        // Verify the operator has no delegations
        let operator_snapshot = ncn_program_client
            .get_operator_snapshot(
                zero_delegation_operator.operator_pubkey,
                ncn_pubkey,
                epoch,
            )
            .await?;

        // Confirm it has zero stake weight
        assert_eq!(
            operator_snapshot.stake_weights().stake_weight(), 0,
            "Zero-delegation operator should have zero stake weight"
        );

        let weather_status = WeatherStatus::Rainy as u8;

        // We expect this to fail due to zero stake
        let result = ncn_program_client
            .do_cast_vote(
                ncn_pubkey,
                zero_delegation_operator.operator_pubkey,
                &zero_delegation_operator.operator_admin,
                weather_status,
                epoch,
            )
            .await;

        // Verify the correct error is returned
        assert_ncn_program_error(result, NCNProgramError::CannotVoteWithZeroStake);
    }
```

This critical security test verifies that:
1. The operator without delegations has a recorded stake weight of zero in its operator snapshot
2. When this zero-stake operator attempts to vote, the transaction fails with a specific error
3. The system correctly enforces the rule that only operators with actual stake can influence consensus

This security mechanism prevents Sybil attacks where an attacker might create many operators without stake to try to influence voting outcomes. The stake-weighted voting system ensures that voting power is proportional to economic commitment.

#### 6.3 Distributing Votes Across Different Options

```rust
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
```

This section demonstrates the system's ability to handle diverse voting preferences:
1. The first operator votes for "Cloudy" (representing a minority view)
2. The second and third operators vote for "Sunny" (the presumed majority view)
3. Each `do_cast_vote` call invokes the NCN program with the operator's choice

Under the hood, each vote triggers several key operations:
- The system verifies the operator admin's authority to vote on behalf of the operator
- It checks that the operator hasn't already voted in this epoch
- It retrieves the operator's snapshot to determine its voting power
- It records the vote in the ballot box, attributing the appropriate stake weight
- It updates the tally for the chosen option
- It checks whether the new vote has pushed any option past the consensus threshold

#### 6.4 Establishing Consensus Through Majority Voting

```rust
    // All remaining operators vote for Sunny to form a majority
    for operator_root in test_ncn.operators.iter().take(OPERATOR_COUNT - 1).skip(3) {
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

To establish a clear consensus, the remaining operators (excluding the zero-delegation operator) all vote for the "Sunny" option. This creates a supermajority that surpasses the required threshold for consensus.

The consensus mechanism works as follows:
1. The system maintains a running tally of stake weight for each voting option
2. After each vote, it calculates whether any option has reached the consensus threshold (typically 66% of total stake)
3. If an option reaches consensus, the system marks the slot when consensus was achieved
4. Consensus requires a supermajority to ensure that decisions have strong support across the network
5. Once consensus is reached, a record is created that persists even after the voting epoch ends

#### 6.5 Vote Processing Architecture

When an operator casts a vote, the system performs several critical operations to ensure security and proper consensus calculation:

1. **Authentication**: Verifies that the transaction is signed by the operator's admin key
   
2. **Authorization**: Confirms that:
   - The operator exists and has an active relationship with the NCN
   - The operator has not already voted in this epoch
   - The operator has non-zero stake weight

3. **Vote Recording**:
   - Creates an `OperatorVote` record in the ballot box
   - Stores the operator's public key, slot when voted, stake weight, and ballot choice
   - Marks the operator as having voted for this epoch

4. **Ballot Processing**:
   - Updates or creates a `BallotTally` for the chosen option
   - Adds the operator's stake weight to the tally
   - Increments the vote count for this option

5. **Consensus Calculation**:
   - Compares the winning ballot's stake weight against the total possible stake
   - If the winning ballot exceeds the threshold (e.g., 66%), marks consensus as reached
   - Records the slot when consensus was reached
   - Creates a `ConsensusResult` account to permanently record the outcome

6. **Cross-Validation**:
   - Ensures the vote is being cast within the correct epoch
   - Verifies the operator's snapshot exists and contains valid data
   - Checks that the epoch state allows voting at this stage

This multi-layered architecture ensures votes are processed securely, tallied correctly, and that consensus is determined accurately based on stake-weighted participation.

#### 6.6 Security Considerations in the Voting Process

The voting process incorporates several key security features:

1. **Sybil Attack Prevention**: 
   - Voting power is based on stake weight, not operator count
   - Zero-stake operators cannot participate, preventing fake operator attacks

2. **Replay Protection**:
   - Each operator can only vote once per epoch
   - The system tracks which operators have already voted

3. **Time-Bound Voting**:
   - Votes are only accepted within the appropriate epoch
   - After consensus is reached, there's a limited window for additional votes

4. **Authority Verification**:
   - Only the designated operator admin can cast votes for an operator
   - Signature verification ensures proper authorization

5. **Tamper-Proof Tallying**:
   - Votes are tallied based on immutable snapshot data
   - The system prevents retroactive changes to stake weights

6. **Dynamic Threshold Adaptation**:
   - Consensus threshold is calculated based on the total recorded stake
   - This adapts automatically as the network grows or contracts

These security measures ensure the voting process remains resilient against various attack vectors and manipulation attempts, maintaining the integrity of the consensus mechanism.

### 7. Verification

The Verification phase validates that the voting process completed successfully and that the expected consensus was achieved. This critical step confirms the integrity of the entire system by examining the on-chain data structures and verifying they contain the expected results.

#### 7.1 Ballot Box Verification

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

The first verification step examines the ballot box account:

1. **Winning Ballot Check**:
   - `has_winning_ballot()` confirms that a valid winning ballot was identified
   - This means at least one valid weather status received votes
   - A winning ballot must exceed the required consensus threshold

2. **Consensus Status Check**:
   - `is_consensus_reached()` verifies that the winning ballot achieved the required supermajority
   - The consensus threshold is typically set at 66% of total stake weight
   - This confirms that the voting process successfully reached a definitive conclusion

3. **Outcome Verification**:
   - The test confirms that the winning weather status matches the expected "Sunny" status
   - This ensures that the voting and tallying logic correctly identified the majority choice
   - It validates that the stake-weighted voting mechanism worked as designed

The ballot box serves as the primary record of the voting process, capturing all votes cast and the aggregate results. Its verification ensures the core voting mechanism functioned correctly.

#### 7.2 Consensus Result Account Verification

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

The second verification step examines the `ConsensusResult` account, which serves as the permanent, persistent record of the voting outcome:

1. **Consensus Result Existence**:
   - The test confirms that a `ConsensusResult` account was created for this epoch
   - This account is created automatically when consensus is reached
   - It serves as the authoritative record of the voting outcome

2. **Consensus Status Validation**:
   - `is_consensus_reached()` verifies the consensus flag is properly set
   - This confirms the outcome is officially recognized by the system

3. **Metadata Verification**:
   - The epoch field matches the current epoch, confirming proper account initialization
   - The weather status matches the expected "Sunny" value, validating outcome recording

4. **Cross-Account Consistency Check**:
   - The test compares values between the ballot box and consensus result
   - The vote weight in the consensus result must match the stake weight of the winning ballot
   - This ensures consistency between the voting process and the final recorded outcome

5. **Detailed Reporting**:
   - The test outputs detailed information about the consensus result
   - This includes the winning weather status, vote weights, and consensus recorder
   - This information helps with debugging and validation

#### 7.3 Architecture of Verification and Result Persistence

The verification phase highlights several important architectural features of the NCN system:

1. **Dual Record Keeping**:
   - The system maintains two separate records of the outcome:
     - The `BallotBox` account contains the complete voting history and tallies
     - The `ConsensusResult` account provides a persistent record of the outcome
   - This redundancy ensures the outcome remains accessible even after cleanup

2. **Record Separation**:
   - The voting process (ballot box) is separated from the outcome record (consensus result)
   - This separation allows the system to clean up voting data while preserving results
   - It follows the principle of separating process from outcome

3. **Automated Result Creation**:
   - When consensus is reached, the system automatically creates the consensus result
   - This removes the need for a separate administrative action to record the outcome
   - It ensures timely and accurate recording of results

4. **Result Immutability**:
   - Once created, the consensus result cannot be modified
   - This immutability ensures that voting outcomes cannot be tampered with
   - It provides a trustworthy historical record of all past decisions

5. **Time and Slot Tracking**:
   - Both records track timing information such as:
     - The slot when consensus was reached
     - The epoch when the vote occurred
     - The total duration of the voting process
   - This temporal metadata is valuable for system analysis and optimization

#### 7.4 Verification Techniques and Best Practices

The verification approach demonstrates several best practices for validating blockchain-based voting systems:

1. **Multi-Level Verification**:
   - Tests verify both the process (ballot box) and outcome (consensus result)
   - This catches errors that might occur at different stages of the pipeline

2. **Equality Assertions**:
   - Key values are compared using strict equality assertions
   - This ensures exact matching rather than approximate validation

3. **Cross-Structure Validation**:
   - Values are compared across different accounts to ensure consistency
   - This validates that data propagated correctly between system components

4. **Complete Outcome Validation**:
   - Tests check not just the winning choice, but also:
     - The stake weights behind the decision
     - The consensus status flags
     - The epoch and metadata values
   - This comprehensive approach catches subtle integration issues

5. **Detailed Reporting**:
   - The test outputs human-readable verification results
   - This helps with debugging and provides clear validation evidence

The verification phase is critical to ensuring the entire voting pipeline works correctly, from initialization through voting to final consensus recording. By thoroughly validating all aspects of the process, it confirms the system's ability to securely and accurately reach and record consensus decisions.

### 8. Cleanup

After the test completes, the accounts are cleaned up:

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

This cleanup:
1. Records the current epoch
2. Closes all epoch-related accounts
3. Verifies that the consensus result account persists (as it contains the final result)

## Detailed Function Explanations

This section provides in-depth explanations of the key functions used in the simulation test, their parameters, and their internal workings.

### Core Setup Functions

#### `TestBuilder::new()`

```rust
pub async fn new() -> Self {
    let program_test = ProgramTest::new(
        "ncn_program",
        ncn_program::id(),
        processor!(ncn_program::processor::process_instruction),
    );

    // Add the vault and restaking programs
    let mut program_test = program_test
        .add_program(
            "vault_program",
            vault_program::id(),
            processor!(vault_program::processor::process_instruction),
        )
        .add_program(
            "restaking_program",
            jito_restaking_program::id(),
            processor!(jito_restaking_program::processor::process_instruction),
        );

    // Start the test context
    let mut context = program_test.start_with_context().await;
    
    Self {
        context,
        payer: context.payer.insecure_clone(),
    }
}
```

This function initializes the test environment by:
1. Creating a `ProgramTest` instance for the NCN program
2. Adding the vault and restaking programs to the test environment
3. Starting the test context with a simulated Solana runtime
4. Storing the context and payer keypair for later use

#### `initialize_staking_and_vault_programs()`

```rust
pub async fn initialize_staking_and_vault_programs(&mut self) -> TestResult<()> {
    // Initialize the vault program configuration
    let mut vault_program_client = self.vault_client();
    vault_program_client.do_initialize_config().await?;

    // Initialize the restaking program configuration
    let mut restaking_program_client = self.restaking_program_client();
    restaking_program_client.do_initialize_config().await?;

    Ok(())
}
```

This function:
1. Gets clients for the vault and restaking programs
2. Initializes their configurations with default parameters
3. These configurations are required before any operations can be performed with these programs

### NCN Setup Functions

#### `create_test_ncn()`

```rust
pub async fn create_test_ncn(&mut self) -> TestResult<TestNcn> {
    let mut restaking_program_client = self.restaking_program_client();

    // Create an NCN using the restaking program
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

This function creates a new Node Consensus Network (NCN) by:
1. Getting a client for the restaking program
2. Calling `do_initialize_ncn()` to create an NCN account
3. Returning a `TestNcn` struct with the NCN root and empty lists for operators and vaults

##### `do_initialize_ncn()`

```rust
pub async fn do_initialize_ncn(&mut self, admin: Option<Keypair>) -> TestResult<NcnRoot> {
    // Generate a unique NCN keypair
    let ncn_keypair = Keypair::new();
    let ncn_pubkey = ncn_keypair.pubkey();
    
    // Use provided admin or default to payer
    let ncn_admin = admin.unwrap_or_else(|| self.payer.insecure_clone());
    
    // Find the config address
    let config_address = Config::find_program_address(&jito_restaking_program::id()).0;
    
    // Build the initialize NCN instruction
    let ix = InitializeNcnBuilder::new()
        .config(config_address)
        .ncn(ncn_pubkey)
        .ncn_admin(ncn_admin.pubkey())
        .instruction();
    
    // Process the transaction
    let blockhash = self.banks_client.get_latest_blockhash().await?;
    self.process_transaction(&Transaction::new_signed_with_payer(
        &[ix],
        Some(&self.payer.pubkey()),
        &[&self.payer, &ncn_keypair, &ncn_admin],
        blockhash,
    ))
    .await?;
    
    // Return the NCN root structure
    Ok(NcnRoot {
        ncn_pubkey,
        ncn_keypair,
        ncn_admin,
    })
}
```

This function:
1. Generates a new keypair for the NCN
2. Uses the provided admin keypair or defaults to the test payer
3. Finds the restaking program's config address
4. Creates an instruction to initialize an NCN
5. Processes the transaction with the appropriate signers
6. Returns an `NcnRoot` structure with the NCN's public key, keypair, and admin

#### `setup_ncn_program()`

```rust
pub async fn setup_ncn_program(&mut self, ncn_root: &NcnRoot) -> TestResult<()> {
    // Initialize the NCN program configuration
    self.do_initialize_config(ncn_root.ncn_pubkey, &ncn_root.ncn_admin).await?;
    
    // Initialize the vault registry
    self.do_full_initialize_vault_registry(ncn_root.ncn_pubkey).await?;
    
    Ok(())
}
```

This function configures the NCN program for a specific NCN by:
1. Initializing the NCN program configuration
2. Creating a vault registry to track vaults and token mints
3. This prepares the NCN program to start accepting vault and token registrations

### Operator and Vault Setup Functions

#### `add_operators_to_test_ncn()`

```rust
pub async fn add_operators_to_test_ncn(
    &mut self,
    test_ncn: &mut TestNcn,
    operator_count: usize,
    operator_fees_bps: Option<u16>,
) -> TestResult<()> {
    let mut restaking_program_client = self.restaking_program_client();

    for _ in 0..operator_count {
        // Create a new operator
        let operator_root = restaking_program_client
            .do_initialize_operator(operator_fees_bps)
            .await?;

        // Establish NCN <> operator bidirectional handshake
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

        // Add the operator to the test NCN
        test_ncn.operators.push(operator_root);
    }

    Ok(())
}
```

This function creates and connects multiple operators to an NCN by:
1. Creating each operator with the specified fee in basis points
2. Establishing a bidirectional handshake between each operator and the NCN through:
   - Initializing the NCN-operator state
   - Warming up the NCN's connection to the operator
   - Warming up the operator's connection to the NCN
3. Adding each operator to the `TestNcn` structure for tracking

##### `do_initialize_operator()`

```rust
pub async fn do_initialize_operator(
    &mut self,
    operator_fees_bps: Option<u16>,
) -> TestResult<OperatorRoot> {
    // Generate keypairs for the operator and admin
    let operator_keypair = Keypair::new();
    let operator_pubkey = operator_keypair.pubkey();
    let operator_admin = Keypair::new();
    
    // Find the config address
    let config_address = Config::find_program_address(&jito_restaking_program::id()).0;
    
    // Default fee to 0 if not specified
    let fees_bps = operator_fees_bps.unwrap_or(0);
    
    // Build the initialize operator instruction
    let ix = InitializeOperatorBuilder::new()
        .config(config_address)
        .operator(operator_pubkey)
        .operator_admin(operator_admin.pubkey())
        .fees_bps(fees_bps)
        .instruction();
    
    // Process the transaction
    let blockhash = self.banks_client.get_latest_blockhash().await?;
    self.process_transaction(&Transaction::new_signed_with_payer(
        &[ix],
        Some(&self.payer.pubkey()),
        &[&self.payer, &operator_keypair, &operator_admin],
        blockhash,
    ))
    .await?;
    
    // Return the operator root structure
    Ok(OperatorRoot {
        operator_pubkey,
        operator_keypair,
        operator_admin,
    })
}
```

This function:
1. Generates keypairs for the operator and its admin
2. Finds the restaking program's config address
3. Uses the provided fee or defaults to 0 basis points
4. Creates an instruction to initialize an operator
5. Processes the transaction with the appropriate signers
6. Returns an `OperatorRoot` structure with the operator's public key, keypair, and admin

#### `add_vaults_to_test_ncn()`

```rust
pub async fn add_vaults_to_test_ncn(
    &mut self,
    test_ncn: &mut TestNcn,
    vault_count: usize,
    token_mint: Option<Keypair>,
) -> TestResult<()> {
    let mut vault_program_client = self.vault_program_client();
    let mut restaking_program_client = self.restaking_program_client();

    // Set vault fees to zero for testing
    const DEPOSIT_FEE_BPS: u16 = 0;
    const WITHDRAWAL_FEE_BPS: u16 = 0;
    const REWARD_FEE_BPS: u16 = 0;

    // Use provided token mint or generate a new one
    let should_generate = token_mint.is_none();
    let pass_through = if token_mint.is_some() {
        token_mint.unwrap()
    } else {
        Keypair::new()
    };

    for _ in 0..vault_count {
        // Use the same mint or generate a new one for each vault
        let pass_through = if should_generate {
            Keypair::new()
        } else {
            pass_through.insecure_clone()
        };

        // Initialize the vault
        let vault_root = vault_program_client
            .do_initialize_vault(
                DEPOSIT_FEE_BPS,
                WITHDRAWAL_FEE_BPS,
                REWARD_FEE_BPS,
                9, // Decimals
                &self.context.payer.pubkey(),
                Some(pass_through),
            )
            .await?;

        // Establish vault <> NCN bidirectional handshake
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

        // Add the vault to the test NCN
        test_ncn.vaults.push(vault_root);
    }

    Ok(())
}
```

This function creates and connects multiple vaults to an NCN by:
1. Setting vault fees to zero for testing purposes
2. Using the provided token mint or generating a new one
3. For each vault:
   - Initializing a vault with the specified parameters
   - Establishing a bidirectional handshake between the vault and the NCN through:
     - Initializing the NCN-vault ticket
     - Warming up the NCN's connection to the vault
     - Initializing the vault's connection to the NCN
4. Adding each vault to the `TestNcn` structure for tracking

##### `do_initialize_vault()`

```rust
pub async fn do_initialize_vault(
    &mut self,
    deposit_fee_bps: u16,
    withdrawal_fee_bps: u16,
    reward_fee_bps: u16,
    decimals: u8,
    admin_pubkey: &Pubkey,
    token_mint_keypair: Option<Keypair>,
) -> TestResult<VaultRoot> {
    // Generate a keypair for the vault
    let vault_keypair = Keypair::new();
    let vault_pubkey = vault_keypair.pubkey();
    
    // Use provided token mint or create a new one
    let (token_mint, token_mint_keypair) = if let Some(keypair) = token_mint_keypair {
        let mint = keypair.pubkey();
        (mint, keypair)
    } else {
        let keypair = Keypair::new();
        (keypair.pubkey(), keypair)
    };
    
    // Find the config address
    let config_address = vault_program::config::Config::find_program_address(
        &vault_program::id()
    ).0;
    
    // Build the initialize vault instruction
    let ix = vault_program::instruction::InitializeVaultBuilder::new()
        .config(config_address)
        .vault(vault_pubkey)
        .admin(*admin_pubkey)
        .token_mint(token_mint)
        .deposit_fee_bps(deposit_fee_bps)
        .withdrawal_fee_bps(withdrawal_fee_bps)
        .reward_fee_bps(reward_fee_bps)
        .decimals(decimals)
        .instruction();
    
    // Process the transaction
    let blockhash = self.banks_client.get_latest_blockhash().await?;
    self.process_transaction(&Transaction::new_signed_with_payer(
        &[ix],
        Some(&self.payer.pubkey()),
        &[&self.payer, &vault_keypair],
        blockhash,
    ))
    .await?;
    
    // Return the vault root structure
    Ok(VaultRoot {
        vault_pubkey,
        vault_keypair,
        token_mint,
        token_mint_keypair,
    })
}
```

This function:
1. Generates a keypair for the vault
2. Uses the provided token mint keypair or generates a new one
3. Finds the vault program's config address
4. Creates an instruction to initialize a vault with the specified parameters
5. Processes the transaction with the appropriate signers
6. Returns a `VaultRoot` structure with the vault's public key, keypair, token mint, and token mint keypair

## Expected Outcomes

1. Operators with delegations should successfully cast votes
2. Operators with zero delegations should not be able to vote (returns `CannotVoteWithZeroStake` error)
3. The system should correctly reach consensus with "Sunny" as the winning status
4. All accounts should be properly created and cleaned up
5. The consensus result account should persist after cleaning up other accounts

## Error Cases

The test verifies proper handling of:

1. **Zero delegation operators**: Operators with zero delegations cannot vote
2. **Multiple token types**: The system correctly handles tokens with different weights
3. **Various delegation amounts**: From minimal (1 lamport) to very large (10k tokens)
4. **Split votes**: The system correctly identifies the winning vote with majority support
5. **Account management**: Proper creation and cleanup of all necessary accounts

## Fuzz Testing

The simulation tests also include fuzz testing with randomized configurations:

```rust
struct MintConfig {
    keypair: Keypair,
    weight: u128,       // Weight for voting power calculation
    vault_count: usize, // Number of vaults to create for this mint
}

struct SimConfig {
    operator_count: usize,  // Number of operators to create
    mints: Vec<MintConfig>, // Token mint configurations
    delegations: Vec<u64>,  // Array of delegation amounts for vaults
    operator_fee_bps: u16,  // Operator fee in basis points (100 = 1%)
}

async fn run_simulation(config: SimConfig) -> TestResult<()> {
    // Implementation that runs the simulation with the provided configuration
}

async fn test_basic_simulation() -> TestResult<()> {
    // Basic simulation with standard parameters
}

async fn test_high_operator_count_simulation() -> TestResult<()> {
    // Simulation with a high number of operators
}

async fn test_fuzz_simulation() -> TestResult<()> {
    // Randomized simulation with varying parameters
}
```

These fuzz tests are designed to:

1. Test various combinations of operators, vaults, and token types
2. Verify the system's resilience with different configurations
3. Ensure consensus can be reached across a range of scenarios
4. Identify any edge cases or unexpected behaviors

### NCN Program Configuration Functions

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
        AccountPayer::find_program_address(&ncn_program::id(), &ncn);
    self.airdrop(&account_payer, 100.0).await?;

    let ncn_admin_pubkey = ncn_admin.pubkey();
    self.initialize_config(ncn, ncn_admin, &ncn_admin_pubkey, 3, 10, 10000)
        .await
}
```

This function initializes the NCN program configuration by:
1. Airdrops 1 SOL to the payer account to cover transaction fees
2. Finds the AccountPayer PDA and airdrops 100 SOL to it to cover rent for created accounts
3. Calls `initialize_config()` with specific parameters:
   - 3 epochs before considering a vote stalled
   - 10 epochs after consensus before closing accounts
   - 10000 valid slots after consensus for accepting additional votes

##### `initialize_config()`

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
    // Find the config PDA
    let config = NcnConfig::find_program_address(&ncn_program::id(), &ncn).0;

    // Find the account payer PDA
    let (account_payer, _, _) =
        AccountPayer::find_program_address(&ncn_program::id(), &ncn);

    // Build the initialize config instruction
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

    // Process the transaction
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
1. Finds the NcnConfig PDA address
2. Finds the AccountPayer PDA address
3. Builds an instruction to initialize the NCN program configuration with:
   - The NCN and its admin
   - The account payer for rent
   - The tie breaker admin (who can resolve stalled votes)
   - Timing parameters for stalls, account closing, and vote acceptance
4. Processes the transaction with the NCN admin as the signer

#### `do_full_initialize_vault_registry()`

```rust
pub async fn do_full_initialize_vault_registry(
    &mut self,
    ncn: Pubkey,
) -> TestResult<()> {
    // Find the vault registry PDA
    let (vault_registry, _, _) = VaultRegistry::find_program_address(&ncn_program::id(), &ncn);
    
    // Find the config PDA
    let (ncn_config, _, _) = NcnConfig::find_program_address(&ncn_program::id(), &ncn);
    
    // Build the initialize vault registry instruction
    let ix = InitializeVaultRegistryBuilder::new()
        .vault_registry(vault_registry)
        .config(ncn_config)
        .ncn(ncn)
        .instruction();
    
    // Process the transaction
    let blockhash = self.banks_client.get_latest_blockhash().await?;
    self.process_transaction(&Transaction::new_signed_with_payer(
        &[ix],
        Some(&self.payer.pubkey()),
        &[&self.payer],
        blockhash,
    ))
    .await
}
```

This function:
1. Finds the VaultRegistry PDA address
2. Finds the NcnConfig PDA address
3. Builds an instruction to initialize the vault registry for the NCN
4. Processes the transaction with the payer as the signer
5. The vault registry is a critical component that tracks all supported vaults and token mints

#### `do_admin_register_st_mint()`

```rust
pub async fn do_admin_register_st_mint(
    &mut self,
    ncn: Pubkey,
    st_mint: Pubkey,
    weight: u128,
) -> TestResult<()> {
    // Find the vault registry PDA
    let vault_registry =
        VaultRegistry::find_program_address(&ncn_program::id(), &ncn).0;

    // Find the config PDA
    let (ncn_config, _, _) =
        NcnConfig::find_program_address(&ncn_program::id(), &ncn);

    // Get the admin (payer in this context)
    let admin = self.payer.pubkey();

    // Register the ST mint with the specified weight
    self.admin_register_st_mint(ncn, ncn_config, vault_registry, admin, st_mint, weight)
        .await
}
```

This function registers a Supported Token (ST) mint with a specific weight by:
1. Finding the vault registry and config PDAs
2. Using the payer as the admin (must be the NCN admin in production)
3. Calling `admin_register_st_mint()` with all necessary parameters

##### `admin_register_st_mint()`

```rust
pub async fn admin_register_st_mint(
    &mut self,
    ncn: Pubkey,
    config: Pubkey,
    vault_registry: Pubkey,
    admin: Pubkey,
    st_mint: Pubkey,
    weight: u128,
) -> TestResult<()> {
    // Build the admin register ST mint instruction
    let ix = AdminRegisterStMintBuilder::new()
        .config(config)
        .vault_registry(vault_registry)
        .ncn(ncn)
        .admin(admin)
        .st_mint(st_mint)
        .weight(weight)
        .instruction();
    
    // Process the transaction
    let blockhash = self.banks_client.get_latest_blockhash().await?;
    self.process_transaction(&Transaction::new_signed_with_payer(
        &[ix],
        Some(&self.payer.pubkey()),
        &[&self.payer],
        blockhash,
    ))
    .await
}
```

This function:
1. Builds an instruction to register an ST mint with the specified weight
2. Processes the transaction with the payer as the signer
3. This adds the token mint to the vault registry with its corresponding weight
4. The weight will be used as a multiplier for delegations in this token type

#### `do_register_vault()`

```rust
pub async fn do_register_vault(
    &mut self,
    ncn: Pubkey,
    vault: Pubkey,
    ncn_vault_ticket: Pubkey,
) -> TestResult<()> {
    // Find the vault registry PDA
    let vault_registry =
        VaultRegistry::find_program_address(&ncn_program::id(), &ncn).0;
    
    // Find the config PDA
    let (ncn_config, _, _) =
        NcnConfig::find_program_address(&ncn_program::id(), &ncn);
    
    // Build the register vault instruction
    let ix = RegisterVaultBuilder::new()
        .config(ncn_config)
        .vault_registry(vault_registry)
        .ncn(ncn)
        .vault(vault)
        .ncn_vault_ticket(ncn_vault_ticket)
        .instruction();
    
    // Process the transaction
    let blockhash = self.banks_client.get_latest_blockhash().await?;
    self.process_transaction(&Transaction::new_signed_with_payer(
        &[ix],
        Some(&self.payer.pubkey()),
        &[&self.payer],
        blockhash,
    ))
    .await
}
```

This function registers a vault with the NCN program by:
1. Finding the vault registry and config PDAs
2. Building an instruction to register the vault with its NCN vault ticket
3. Processing the transaction with the payer as the signer
4. This adds the vault to the vault registry, allowing it to participate in the voting system

### Epoch Snapshot and Voting Preparation Functions

#### `add_epoch_state_for_test_ncn()`

```rust
pub async fn add_epoch_state_for_test_ncn(&mut self, test_ncn: &TestNcn) -> TestResult<()> {
    let mut ncn_program_client = self.ncn_program_client();

    // Advance time to ensure we're in a new epoch
    self.warp_slot_incremental(1000).await?;

    // Get the current epoch
    let clock = self.clock().await;
    let epoch = clock.epoch;
    
    // Initialize the epoch state
    ncn_program_client
        .do_intialize_epoch_state(test_ncn.ncn_root.ncn_pubkey, epoch)
        .await?;

    Ok(())
}
```

This function initializes an epoch state for the current epoch by:
1. Advancing time by 1000 slots to ensure we're in a new epoch
2. Getting the current epoch from the clock
3. Calling `do_intialize_epoch_state()` to create an epoch state account
4. The epoch state tracks the progress of the voting cycle for this epoch

##### `do_intialize_epoch_state()`

```rust
pub async fn do_intialize_epoch_state(&mut self, ncn: Pubkey, epoch: u64) -> TestResult<()> {
    // Find the epoch state PDA
    let epoch_state = EpochState::find_program_address(&ncn_program::id(), &ncn, epoch).0;
    
    // Find the config PDA
    let (ncn_config, _, _) = NcnConfig::find_program_address(&ncn_program::id(), &ncn);
    
    // Build the initialize epoch state instruction
    let ix = InitializeEpochStateBuilder::new()
        .epoch_state(epoch_state)
        .config(ncn_config)
        .ncn(ncn)
        .epoch(epoch)
        .instruction();
    
    // Process the transaction
    let blockhash = self.banks_client.get_latest_blockhash().await?;
    self.process_transaction(&Transaction::new_signed_with_payer(
        &[ix],
        Some(&self.payer.pubkey()),
        &[&self.payer],
        blockhash,
    ))
    .await
}
```

This function:
1. Finds the EpochState PDA address for the specific NCN and epoch
2. Finds the NcnConfig PDA address
3. Builds an instruction to initialize an epoch state account
4. Processes the transaction with the payer as the signer
5. The epoch state tracks which stage of the voting cycle we're in

#### `add_weights_for_test_ncn()`

```rust
pub async fn add_weights_for_test_ncn(&mut self, test_ncn: &TestNcn) -> TestResult<()> {
    let mut ncn_program_client = self.ncn_program_client();

    // Get the current epoch
    let clock = self.clock().await;
    let epoch = clock.epoch;
    
    // Initialize the weight table
    ncn_program_client
        .do_full_initialize_weight_table(test_ncn.ncn_root.ncn_pubkey, epoch)
        .await?;

    // Set the epoch weights
    ncn_program_client
        .do_set_epoch_weights(test_ncn.ncn_root.ncn_pubkey, epoch)
        .await?;

    Ok(())
}
```

This function sets up token weights for the current epoch by:
1. Getting the current epoch from the clock
2. Calling `do_full_initialize_weight_table()` to create a weight table
3. Calling `do_set_epoch_weights()` to copy weights from the vault registry to the weight table
4. This process creates a snapshot of token weights for the current voting cycle

##### `do_set_epoch_weights()`

```rust
pub async fn do_set_epoch_weights(&mut self, ncn: Pubkey, epoch: u64) -> TestResult<()> {
    // Find the epoch state PDA
    let epoch_state = EpochState::find_program_address(&ncn_program::id(), &ncn, epoch).0;
    
    // Find the config PDA
    let (ncn_config, _, _) = NcnConfig::find_program_address(&ncn_program::id(), &ncn);
    
    // Find the vault registry PDA
    let vault_registry = VaultRegistry::find_program_address(&ncn_program::id(), &ncn).0;
    
    // Find the weight table PDA
    let weight_table = WeightTable::find_program_address(&ncn_program::id(), &ncn, epoch).0;
    
    // Build the set epoch weights instruction
    let ix = SetEpochWeightsBuilder::new()
        .epoch_state(epoch_state)
        .config(ncn_config)
        .vault_registry(vault_registry)
        .weight_table(weight_table)
        .ncn(ncn)
        .epoch(epoch)
        .instruction();
    
    // Process the transaction
    let blockhash = self.banks_client.get_latest_blockhash().await?;
    self.process_transaction(&Transaction::new_signed_with_payer(
        &[ix],
        Some(&self.payer.pubkey()),
        &[&self.payer],
        blockhash,
    ))
    .await
}
```

This function:
1. Finds all necessary PDA addresses (epoch state, config, vault registry, weight table)
2. Builds an instruction to set epoch weights by copying from the vault registry to the weight table
3. Processes the transaction with the payer as the signer
4. This creates a snapshot of token weights that will be used for this voting cycle

#### `add_epoch_snapshot_to_test_ncn()`

```rust
pub async fn add_epoch_snapshot_to_test_ncn(&mut self, test_ncn: &TestNcn) -> TestResult<()> {
    let mut ncn_program_client = self.ncn_program_client();

    // Get the current epoch
    let clock = self.clock().await;
    let epoch = clock.epoch;
    
    // Find the epoch state PDA
    let epoch_state = EpochState::find_program_address(
        &ncn_program::id(),
        &test_ncn.ncn_root.ncn_pubkey,
        epoch,
    ).0;
    
    // Get the epoch state to verify we're at the right stage
    let epoch_state_account = ncn_program_client
        .get_epoch_state(test_ncn.ncn_root.ncn_pubkey, epoch)
        .await?;
    
    // Ensure weights are set before taking snapshot
    assert!(epoch_state_account.set_weight_progress().is_complete());
    
    // Initialize the epoch snapshot
    ncn_program_client
        .do_initialize_epoch_snapshot(test_ncn.ncn_root.ncn_pubkey, epoch)
        .await?;

    Ok(())
}
```

This function creates an aggregate epoch snapshot by:
1. Getting the current epoch from the clock
2. Finding the epoch state PDA address
3. Verifying that weights have been set (weight setting must be complete)
4. Calling `do_initialize_epoch_snapshot()` to create an epoch snapshot account
5. This snapshot captures the total state of the system for this epoch

##### `do_initialize_epoch_snapshot()`

```rust
pub async fn do_initialize_epoch_snapshot(&mut self, ncn: Pubkey, epoch: u64) -> TestResult<()> {
    // Find the epoch state PDA
    let epoch_state = EpochState::find_program_address(&ncn_program::id(), &ncn, epoch).0;
    
    // Find the config PDA
    let (ncn_config, _, _) = NcnConfig::find_program_address(&ncn_program::id(), &ncn);
    
    // Find the epoch snapshot PDA
    let epoch_snapshot = EpochSnapshot::find_program_address(&ncn_program::id(), &ncn, epoch).0;
    
    // Find the weight table PDA
    let weight_table = WeightTable::find_program_address(&ncn_program::id(), &ncn, epoch).0;
    
    // Find the vault registry PDA
    let vault_registry = VaultRegistry::find_program_address(&ncn_program::id(), &ncn).0;
    
    // Build the initialize epoch snapshot instruction
    let ix = InitializeEpochSnapshotBuilder::new()
        .epoch_state(epoch_state)
        .config(ncn_config)
        .epoch_snapshot(epoch_snapshot)
        .weight_table(weight_table)
        .vault_registry(vault_registry)
        .ncn(ncn)
        .epoch(epoch)
        .instruction();
    
    // Process the transaction
    let blockhash = self.banks_client.get_latest_blockhash().await?;
    self.process_transaction(&Transaction::new_signed_with_payer(
        &[ix],
        Some(&self.payer.pubkey()),
        &[&self.payer],
        blockhash,
    ))
    .await
}
```

This function:
1. Finds all necessary PDA addresses (epoch state, config, epoch snapshot, weight table, vault registry)
2. Builds an instruction to initialize an epoch snapshot account
3. Processes the transaction with the payer as the signer
4. The epoch snapshot aggregates system-wide metrics like total stake and participant counts

#### `add_operator_snapshots_to_test_ncn()`

```rust
pub async fn add_operator_snapshots_to_test_ncn(&mut self, test_ncn: &TestNcn) -> TestResult<()> {
    let mut ncn_program_client = self.ncn_program_client();

    // Get the current epoch
    let clock = self.clock().await;
    let epoch = clock.epoch;
    
    // Create a snapshot for each operator
    for operator_root in test_ncn.operators.iter() {
        ncn_program_client
            .do_initialize_operator_snapshot(
                test_ncn.ncn_root.ncn_pubkey,
                operator_root.operator_pubkey,
                epoch,
            )
            .await?;
    }

    Ok(())
}
```

This function creates snapshots for each operator by:
1. Getting the current epoch from the clock
2. Iterating through each operator in the test NCN
3. Calling `do_initialize_operator_snapshot()` for each operator
4. These snapshots record each operator's delegated stake at this point in time

##### `do_initialize_operator_snapshot()`

```rust
pub async fn do_initialize_operator_snapshot(
    &mut self,
    ncn: Pubkey,
    operator: Pubkey,
    epoch: u64,
) -> TestResult<()> {
    // Find the epoch state PDA
    let epoch_state = EpochState::find_program_address(&ncn_program::id(), &ncn, epoch).0;
    
    // Find the config PDA
    let (ncn_config, _, _) = NcnConfig::find_program_address(&ncn_program::id(), &ncn);
    
    // Find the epoch snapshot PDA
    let epoch_snapshot = EpochSnapshot::find_program_address(&ncn_program::id(), &ncn, epoch).0;
    
    // Find the operator snapshot PDA
    let operator_snapshot = OperatorSnapshot::find_program_address(
        &ncn_program::id(),
        &operator,
        &ncn,
        epoch,
    ).0;
    
    // Build the initialize operator snapshot instruction
    let ix = InitializeOperatorSnapshotBuilder::new()
        .epoch_state(epoch_state)
        .config(ncn_config)
        .epoch_snapshot(epoch_snapshot)
        .operator_snapshot(operator_snapshot)
        .ncn(ncn)
        .operator(operator)
        .epoch(epoch)
        .instruction();
    
    // Process the transaction
    let blockhash = self.banks_client.get_latest_blockhash().await?;
    self.process_transaction(&Transaction::new_signed_with_payer(
        &[ix],
        Some(&self.payer.pubkey()),
        &[&self.payer],
        blockhash,
    ))
    .await
}
```

This function:
1. Finds all necessary PDA addresses (epoch state, config, epoch snapshot, operator snapshot)
2. Builds an instruction to initialize an operator snapshot account
3. Processes the transaction with the payer as the signer
4. The operator snapshot records the operator's current stake weight for voting

#### `add_vault_operator_delegation_snapshots_to_test_ncn()`

```rust
pub async fn add_vault_operator_delegation_snapshots_to_test_ncn(
    &mut self,
    test_ncn: &TestNcn,
) -> TestResult<()> {
    let mut ncn_program_client = self.ncn_program_client();
    let mut vault_program_client = self.vault_client();

    // Get the current epoch
    let clock = self.clock().await;
    let epoch = clock.epoch;
    
    // Process each vault
    for vault_root in test_ncn.vaults.iter() {
        // Get the vault's delegation state
        let delegation_state = vault_program_client
            .get_delegation_state(&vault_root.vault_pubkey)
            .await?;
        
        // Process each delegation for this vault
        for i in 0..delegation_state.delegation_count() {
            // Get the delegation details
            let delegation = delegation_state.get_delegation(i);
            
            // Skip if delegation amount is zero
            if delegation.amount() == 0 {
                continue;
            }
            
            // Take a snapshot of this delegation
            ncn_program_client
                .do_snapshot_vault_operator_delegation(
                    test_ncn.ncn_root.ncn_pubkey,
                    vault_root.vault_pubkey,
                    delegation.operator(),
                    epoch,
                )
                .await?;
        }
    }

    Ok(())
}
```

This function captures all vault-to-operator delegations by:
1. Getting the current epoch from the clock
2. Iterating through each vault in the test NCN
3. Getting the vault's delegation state to see which operators it delegates to
4. For each non-zero delegation, calling `do_snapshot_vault_operator_delegation()`
5. This creates a detailed record of exactly how much each vault delegated to each operator

##### `do_snapshot_vault_operator_delegation()`

```rust
pub async fn do_snapshot_vault_operator_delegation(
    &mut self,
    ncn: Pubkey,
    vault: Pubkey,
    operator: Pubkey,
    epoch: u64,
) -> TestResult<()> {
    // Find the epoch state PDA
    let epoch_state = EpochState::find_program_address(&ncn_program::id(), &ncn, epoch).0;
    
    // Find the config PDA
    let (ncn_config, _, _) = NcnConfig::find_program_address(&ncn_program::id(), &ncn);
    
    // Find the vault registry PDA
    let vault_registry = VaultRegistry::find_program_address(&ncn_program::id(), &ncn).0;
    
    // Find the weight table PDA
    let weight_table = WeightTable::find_program_address(&ncn_program::id(), &ncn, epoch).0;
    
    // Find the epoch snapshot PDA
    let epoch_snapshot = EpochSnapshot::find_program_address(&ncn_program::id(), &ncn, epoch).0;
    
    // Find the operator snapshot PDA
    let operator_snapshot = OperatorSnapshot::find_program_address(
        &ncn_program::id(),
        &operator,
        &ncn,
        epoch,
    ).0;
    
    // Find the vault delegation snapshot PDA
    let (delegation_snapshot, _, _) = VaultOperatorDelegationSnapshot::find_program_address(
        &ncn_program::id(),
        &vault,
        &operator,
        &ncn,
        epoch,
    );
    
    // Build the snapshot vault operator delegation instruction
    let ix = SnapshotVaultOperatorDelegationBuilder::new()
        .epoch_state(epoch_state)
        .config(ncn_config)
        .vault_registry(vault_registry)
        .weight_table(weight_table)
        .epoch_snapshot(epoch_snapshot)
        .operator_snapshot(operator_snapshot)
        .delegation_snapshot(delegation_snapshot)
        .ncn(ncn)
        .vault(vault)
        .operator(operator)
        .epoch(epoch)
        .instruction();
    
    // Process the transaction
    let blockhash = self.banks_client.get_latest_blockhash().await?;
    self.process_transaction(&Transaction::new_signed_with_payer(
        &[ix],
        Some(&self.payer.pubkey()),
        &[&self.payer],
        blockhash,
    ))
    .await
}
```

This function:
1. Finds all necessary PDA addresses for the involved accounts
2. Builds an instruction to snapshot a specific vault-operator delegation
3. Processes the transaction with the payer as the signer
4. This creates a detailed record of a single delegation, including its amount and weight

#### `add_ballot_box_to_test_ncn()`

```rust
pub async fn add_ballot_box_to_test_ncn(&mut self, test_ncn: &TestNcn) -> TestResult<()> {
    let mut ncn_program_client = self.ncn_program_client();

    // Get the current epoch
    let clock = self.clock().await;
    let epoch = clock.epoch;
    let ncn = test_ncn.ncn_root.ncn_pubkey;

    // Initialize the ballot box
    ncn_program_client
        .do_full_initialize_ballot_box(ncn, epoch)
        .await?;

    Ok(())
}
```

This function creates a ballot box for collecting votes by:
1. Getting the current epoch from the clock
2. Calling `do_full_initialize_ballot_box()` to create a ballot box account
3. The ballot box is where votes are collected and tallied during the voting process

##### `do_full_initialize_ballot_box()`

```rust
pub async fn do_full_initialize_ballot_box(&mut self, ncn: Pubkey, epoch: u64) -> TestResult<()> {
    // Find the epoch state PDA
    let epoch_state = EpochState::find_program_address(&ncn_program::id(), &ncn, epoch).0;
    
    // Find the config PDA
    let (ncn_config, _, _) = NcnConfig::find_program_address(&ncn_program::id(), &ncn);
    
    // Find the ballot box PDA
    let ballot_box = BallotBox::find_program_address(&ncn_program::id(), &ncn, epoch).0;
    
    // Build the initialize ballot box instruction
    let ix = InitializeBallotBoxBuilder::new()
        .epoch_state(epoch_state)
        .config(ncn_config)
        .ballot_box(ballot_box)
        .ncn(ncn)
        .epoch(epoch)
        .instruction();
    
    // Process the transaction
    let blockhash = self.banks_client.get_latest_blockhash().await?;
    self.process_transaction(&Transaction::new_signed_with_payer(
        &[ix],
        Some(&self.payer.pubkey()),
        &[&self.payer],
        blockhash,
    ))
    .await
}
```

This function:
1. Finds all necessary PDA addresses (epoch state, config, ballot box)
2. Builds an instruction to initialize a ballot box account
3. Processes the transaction with the payer as the signer
4. The ballot box will store all votes and track the consensus status

### Voting and Verification Functions

#### `do_cast_vote()`

```rust
pub async fn do_cast_vote(
    &mut self,
    ncn: Pubkey,
    operator: Pubkey,
    operator_admin: &Keypair,
    weather_status: u8,
    epoch: u64,
) -> TestResult<()> {
    // Find all necessary PDA addresses
    let epoch_state =
        EpochState::find_program_address(&ncn_program::id(), &ncn, epoch).0;
    let ncn_config =
        NcnConfig::find_program_address(&ncn_program::id(), &ncn).0;
    let ballot_box =
        BallotBox::find_program_address(&ncn_program::id(), &ncn, epoch).0;
    let epoch_snapshot =
        EpochSnapshot::find_program_address(&ncn_program::id(), &ncn, epoch).0;
    let operator_snapshot =
        OperatorSnapshot::find_program_address(&ncn_program::id(),
                                              &operator, &ncn, epoch).0;

    // Build the cast vote instruction
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

    // Process the transaction
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

This function casts a vote on behalf of an operator by:
1. Finding all necessary PDA addresses for the involved accounts
2. Building a cast vote instruction with the operator's choice of weather status
3. Processing the transaction with the payer and operator admin as signers
4. This records the operator's vote in the ballot box and updates tallies

#### `close_epoch_accounts_for_test_ncn()`

```rust
pub async fn close_epoch_accounts_for_test_ncn(&mut self, test_ncn: &TestNcn) -> TestResult<()> {
    let mut ncn_program_client = self.ncn_program_client();

    // Get the current epoch
    let clock = self.clock().await;
    let epoch = clock.epoch;
    
    // Get the epoch state
    let epoch_state = ncn_program_client
        .get_epoch_state(test_ncn.ncn_root.ncn_pubkey, epoch)
        .await?;
    
    // Close each type of epoch account
    ncn_program_client
        .do_close_epoch_accounts(
            test_ncn.ncn_root.ncn_pubkey,
            epoch,
            CloseAccountType::WeightTable,
        )
        .await?;
        
    ncn_program_client
        .do_close_epoch_accounts(
            test_ncn.ncn_root.ncn_pubkey,
            epoch,
            CloseAccountType::VaultOperatorDelegationSnapshots,
        )
        .await?;
        
    ncn_program_client
        .do_close_epoch_accounts(
            test_ncn.ncn_root.ncn_pubkey,
            epoch,
            CloseAccountType::OperatorSnapshots,
        )
        .await?;
        
    ncn_program_client
        .do_close_epoch_accounts(
            test_ncn.ncn_root.ncn_pubkey,
            epoch,
            CloseAccountType::EpochSnapshot,
        )
        .await?;
        
    ncn_program_client
        .do_close_epoch_accounts(
            test_ncn.ncn_root.ncn_pubkey,
            epoch,
            CloseAccountType::BallotBox,
        )
        .await?;
        
    ncn_program_client
        .do_close_epoch_accounts(
            test_ncn.ncn_root.ncn_pubkey,
            epoch,
            CloseAccountType::EpochState,
        )
        .await?;

    Ok(())
}
```

This function cleans up all epoch-related accounts by:
1. Getting the current epoch from the clock
2. Getting the epoch state to verify it's safe to close accounts
3. Calling `do_close_epoch_accounts()` for each type of account:
   - Weight table
   - Vault-operator delegation snapshots
   - Operator snapshots
   - Epoch snapshot
   - Ballot box
   - Epoch state
4. This reclaims rent from temporary accounts while preserving the consensus result

##### `do_close_epoch_accounts()`

```rust
pub async fn do_close_epoch_accounts(
    &mut self,
    ncn: Pubkey,
    epoch: u64,
    account_type: CloseAccountType,
) -> TestResult<()> {
    // Find the config PDA
    let (ncn_config, _, _) = NcnConfig::find_program_address(&ncn_program::id(), &ncn);
    
    // Get the account payer (for rent refund)
    let (account_payer, _, _) = AccountPayer::find_program_address(&ncn_program::id(), &ncn);
    
    // Build the close epoch account instruction
    let ix = CloseEpochAccountBuilder::new()
        .config(ncn_config)
        .account_payer(account_payer)
        .ncn(ncn)
        .epoch(epoch)
        .account_type(account_type as u8)
        .instruction();
    
    // Process the transaction
    let blockhash = self.banks_client.get_latest_blockhash().await?;
    self.process_transaction(&Transaction::new_signed_with_payer(
        &[ix],
        Some(&self.payer.pubkey()),
        &[&self.payer],
        blockhash,
    ))
    .await
}
```

This function:
1. Finds the config and account payer PDAs
2. Builds an instruction to close a specific type of epoch account
3. Processes the transaction with the payer as the signer
4. This returns rent to the account payer while maintaining critical results

### WeatherStatus Enum

The WeatherStatus enum represents the different voting options available to operators:

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

This enum:
- Defines three possible weather conditions (Sunny, Cloudy, Rainy)
- Assigns numeric values (0, 1, 2) to each condition
- Sets Sunny as the default option
- In a real-world application, this would be replaced with meaningful decision options
