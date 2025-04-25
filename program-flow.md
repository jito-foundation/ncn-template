# Network Consensus Node (NCN) Template

## TL;DR

### Complete Workflow

Here's how the process flows in a custom NCN implementation:

1. **Setup Phase**
   - Initialize the NCN, operators, and vaults using Jito programs
   - Establish bidirectional connections between all components
   - Register supported token mints with their respective weights

2. **Configuration**
   - Add stake delegations from vaults to operators
   - Register all vaults with the NCN

3. **Per-Epoch Operations**
   - Initialize epoch state to track progress
   - Create weight table to lock in token weights for the epoch
   - Take system state snapshots (epoch, operators, delegations)
   - Set up ballot box for operators to cast votes with custom vote data
   - Process consensus results according to your NCN's purpose
   - Clean up accounts after consensus to maintain blockchain efficiency

This template provides the foundation for a decentralized consensus mechanism with stake-weighted voting, customizable for your specific use case.

### Security Considerations

When building your custom NCN, consider these security aspects:

1. **Stake Weight Manipulation**: Ensure operators cannot manipulate their stake weight immediately before voting
2. **Vote Timeout**: Implement timeouts to prevent deadlocks if consensus cannot be reached
3. **Admin Controls**: Carefully design admin permissions to avoid centralization risks
4. **Economic Security**: Ensure that economic incentives are properly aligned for all participants

By following this template and adapting it to your specific needs, you can build a secure and efficient Network Consensus Node on Solana using Jito's restaking infrastructure.

## Introduction

This template provides the building blocks for creating and deploying your own custom Network Consensus Node (NCN) using the Jito Restaking program.

### System Architecture

The system consists of several key components that work together:

1. **Network Consensus Node (NCN)**: Central entity that coordinates the overall process
2. **Operators**: Entities that validate and vote on consensus data
3. **Vaults**: Smart contracts that hold staked tokens and connect to operators

## Programs Involved

The NCN architecture relies on multiple Solana programs:

1. **From Jito Labs**:
   - Jito Restaking Program
   - Jito Vault Program
2. **From Solana**:
   - SPL Stake Pool Program
3. **Your Custom Program**:
   - You will need to deploy your own NCN program

## Jito Programs Functionality

Jito programs are used to:

1. Initialize operator and vault accounts that store specific structured data
2. Initialize the NCN account (not the on-chain program you'll deploy, but an account that links your NCN with operators and vaults)
3. Initialize and warm up (activate) connections between all three components:
   - NCN <> Operators: Specifies which operators are connected to the NCN
   - NCN <> Vaults: Specifies which vaults are connected to the NCN
   - Operators <> Vaults: Records how much stake each operator has from each vault (vaults can support multiple operators)

**Note**: Until all three components are connected and all connections are warmed up, the system cannot effectively utilize any stake.

## Your Custom NCN Program

### Initialize the Vault Registry

Your program will create an account to hold the NCN key and two main lists:

1. **st_mint_list**: The supported token (ST) mints used in the system, with their weights. The weight is an arbitrary value that determines how each token compares to others. This could be the token price or any value the admin assigns.
2. **vault_list**: The vaults participating in the system.

```rust
pub struct VaultRegistry {
    /// The NCN the vault registry is associated with
    pub ncn: Pubkey,
    /// The bump seed for the PDA
    pub bump: u8,
    /// The list of supported token (ST) mints
    pub st_mint_list: [StMintEntry; 64],
    /// The list of vaults
    pub vault_list: [VaultEntry; 64],
}

pub struct StMintEntry {
    /// The supported token (ST) mint
    st_mint: Pubkey,
    /// The weight value
    weight: PodU128,
}

pub struct VaultEntry {
    /// The vault account
    vault: Pubkey,
    /// The supported token (ST) mint of the vault
    st_mint: Pubkey,
    /// The index of the vault relative to the NCN account
    vault_index: PodU64,
    /// The slot when the vault was registered
    slot_registered: PodU64,
}
```

### Snapshot Process

#### Initialize the Epoch State

The epoch state tracks the current status of the epoch and holds the following data:

- operator_count
- vault_count
- account_status: Indicates whether accounts are closed or not
- set_weight_progress
- epoch_snapshot_progress
- operator_snapshot_progress
- voting_progress
- slot_consensus_reached
- was_tie_breaker_set

#### Set Weights

This step creates a weight table for all mints and vaults associated with the NCN. It runs once before each voting cycle for two main reasons:

- Lock the vaults participating in this vote
- Lock the weights, especially important if the NCN uses token prices as weights, which need updating before each vote

```rust
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
    vault_registry: [VaultEntry; 64],
    /// The weight table
    table: [WeightEntry; 64],
}

pub struct WeightEntry {
    /// Info about the ST mint
    st_mint_entry: StMintEntry,
    /// The weight of the ST mint
    weight: PodU128,
    /// The slot the weight was set
    slot_set: PodU64,
    /// The slot the weight was last updated
    slot_updated: PodU64,
}
```

#### Create Epoch Snapshot

After setting weights, an epoch snapshot captures the system state at this point in time. This account stores information about operators, vaults, and the total stake weights for this epoch.

```rust
pub struct EpochSnapshot {
    /// The NCN the account is associated with
    ncn: Pubkey,
    /// The epoch the account is associated with
    epoch: PodU64,
    /// Bump seed for the PDA
    bump: u8,
    /// Slot the snapshot was created
    slot_created: PodU64,
    /// Slot the snapshot was finalized
    slot_finalized: PodU64,
    /// Number of operators at the time of creation
    operator_count: PodU64,
    /// Number of vaults at the time of creation
    vault_count: PodU64,
    /// Number of operators registered
    operators_registered: PodU64,
    /// Number of valid operator vault delegations
    valid_operator_vault_delegations: PodU64,
    /// Stake weight information
    stake_weights: StakeWeights,
}
```

#### Initialize Operator Snapshots

For each operator connected to the NCN, an operator snapshot is created to capture the operator's current status, delegation amounts, and other relevant voting data.

```rust
pub struct OperatorSnapshot {
    /// The operator this account is for
    operator: Pubkey,
    /// The NCN the account is associated with
    ncn: Pubkey,
    /// The epoch the account is associated with
    epoch: PodU64,
    /// Bump seed for the PDA
    bump: u8,
    /// Slot the snapshot was created
    slot_created: PodU64,
    /// Slot the snapshot was finalized
    slot_finalized: PodU64,
    /// Whether the operator is finalized
    is_finalized: PodBool,
    /// Number of delegations for this operator
    vault_count: PodU16,
    /// Mapping of vault index to delegations
    vault_delegations: [OperatorVaultDelegation; 64],
    /// Stake weighted vote fraction for this operator
    stake_weighted_vote_fraction: PodU64,
}
```

#### Record Vault-Operator Delegations

The final part of the snapshot process records the delegation relationships between vaults and operators. For each operator-vault pair, the system records the delegated stake amount.

### Voting and Consensus Mechanism

After completing the snapshot process, the system enters the voting phase. This is where your custom NCN logic has the most impact, as you decide what operators vote on and how consensus is reached.

#### Initialize Ballot Box

The ballot box is the central account for the voting process, tracking all operator votes and tallying them according to stake weight.

```rust
pub struct BallotBox {
    /// The NCN the account is associated with
    ncn: Pubkey,
    /// The epoch the account is associated with
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

#### Cast Votes

Operators cast votes as ballots containing their chosen vote data. In the Jito Tip Router example, this is represented by a simple `WeatherStatus`, but in a real implementation, this could be a hash of proposed data, transaction, or any other consensus item your NCN requires.

```rust
pub struct Ballot {
    /// The vote data (in the example, weather status)
    vote_data: u8,
    /// Whether the ballot is valid
    is_valid: PodBool,
}
```

When an operator casts a vote, the system considers their stake weight and tallies the vote in the ballot box. Consensus is reached when votes representing at least 66% of the total stake weight agree on the same ballot.

#### Determine Consensus

The system automatically checks if consensus has been reached after each vote:

```rust
// Check the ballot box after votes are cast
let ballot_box = get_ballot_box(ncn, epoch).await?;

if ballot_box.is_consensus_reached() {
    let winning_ballot = ballot_box.get_winning_ballot().unwrap();
    // Process the winning ballot data
    let vote_data = winning_ballot.vote_data();
    // Your custom logic to handle consensus result
}
```

### Account Cleanup and Epoch Progression

After reaching consensus and waiting for a specified period, accounts for the epoch can be closed to reclaim rent. This happens in the reverse order of creation:

1. Close Ballot Box
2. Close Operator Snapshots
3. Close Epoch Snapshot
4. Close Weight Table
5. Close Epoch State

### Implementing Your Custom NCN Logic

To build your own NCN, you'll need to:

1. Define what operators vote on (replace the `WeatherStatus` with your own vote data)
2. Determine how to utilize consensus results
3. Build necessary off-chain infrastructure to support your NCN's use case
4. Implement custom reward distribution logic if needed
