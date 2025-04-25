# NCN Template

## TL;DR

### Complete Workflow Example

Here's how the entire process would look in your custom NCN implementation:

1. **Setup Phase**

   - Initialize the NCN, operators, and vaults using Jito programs
   - Connect all components with bidirectional relationships
   - Register supported token mints and their weights

2. **Configuration**

   - Add stake delegations from vaults to operators
   - Register all vaults with the NCN

3. **Per-Epoch Operations**
   - Initialize epoch state to track progress
   - Create weight table to lock in token weights for the epoch
   - Take snapshots of the system state (epoch snapshot, operator snapshots, delegations)
   - Setup ballot box and enable operators to cast votes with your custom vote data
   - Process consensus results according to your NCN's purpose
   - Close accounts after consensus to maintain blockchain efficiency

This template provides the foundation for a decentralized consensus mechanism with stake-weighted voting, which you can customize for your specific use case.

### Security Considerations

When building your custom NCN, consider these security aspects:

1. **Stake Weight Manipulation**: Ensure operators cannot manipulate their stake weight right before voting
2. **Vote Timeout**: Implement timeouts to prevent deadlocks if consensus cannot be reached
3. **Admin Controls**: Carefully design admin permissions to avoid centralization risks
4. **Economic Security**: Ensure the economic incentives are properly aligned for all participants

By following this template and adapting it to your specific needs, you can build a secure and efficient Network Consensus Node on Solana using Jito's restaking infrastructure.

This template is meant to be the building blocks for creating and deploying your
own custom NCN using Jito Restaking program.

### System Architecture

The system consists of several key components that work together:

1. **Network consensus Node (NCN)**: Central entity that coordinates the overall process
2. **Operators**: Entities that validate and vote on tip distribution data
3. **Vaults**: Smart contracts that hold staked tokens and connect to operators

## programs involved in the system

1. from Jito labs:
   1. jito restaking program
   1. jito vault program
1. from Solana:
   1. spl stake pool program
1. and You will have to deploy your own NCN program

## Jito programs will be used to

1. initialize the operators and vaults accounts, those are only accounts that are going to be set in a specific way and hold a specific data.
1. initialize the NCN account, this is not the onchain program that you are going to deploy, this is only an account used to link your NCN with operators and vaults.
1. initialize and warmup (activate) connections between all the three components (NCN, operators and vaults).
   1. NCN <> operators: to tell which operators are connected to the NCN.
   1. NCN <> vaults: to tell which valut
   1. operators <> vaults: to tell how much stake each operator have from each vault, the vault could support multiple operators.

Note: untill you have all the three components connected, and all the connections warmed up, you can't say that you have any stake

## The NCN program will be used to

### initialize the vault registry

An account that will hold the NCN key, as well as two main lists.

1.  st_mint_list: the ST mints (support mints) that are going to be used in the system, with thier weights. The weight is an aribuitrary value to determin how this specific token could be compared with other tokens supported by the system, you can set that to be the token price itself, or just a value the admin what to put for it.
1.  vault_list: the vaults that are going to be used in the system.

```rust

pub struct VaultRegistry {
    /// The NCN the vault registry is associated with
    pub ncn: Pubkey,
    /// The bump seed for the PDA
    pub bump: u8,
    /// The list of supported token ( ST ) mints
    pub st_mint_list: [StMintEntry; 64],
    /// The list of vaults
    pub vault_list: [VaultEntry; 64],
}

pub struct StMintEntry {
    /// The supported token ( ST ) mint
    st_mint: Pubkey,

    /// The weight when
    weight: PodU128,
}

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

### Starting snapshot process

#### Initialize the epoch state

Epoch state will hold the following data:

- operator_count
- vault_count
- account_status: the status of the accounts, if they are closed or not.
- set_weight_progress
- epoch_snapshot_progress
- operator_snapshot_progress
- voting_progress
- slot_consensus_reached
- was_tie_breaker_set

#### Set weights

This step will take all the mints and vaults associated with the NCN and create a weight table for them, This step will ran once before each vote for two reasons:

- lock the vaults that are going to be in this vote
- lock the weights, specially if the NCN uses the price of the token as a weight, then before each vote, this will have to fetch the new price and update the weights

The weight table structs

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

#### Create Epoch Snapshot

After setting the weights, an epoch snapshot is created to capture the state of the system at this point in time. The epoch snapshot account stores information about the operators, vaults, and the total stake weights at this epoch.

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

For each operator connected to the NCN, an operator snapshot is created. This captures the operator's current status, delegation amounts, and other relevant data for the voting process.

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

#### Snapshot Vault-Operator Delegations

The final part of the snapshot process is to record the delegation relationships between vaults and operators. For each operator-vault pair, the system records how much stake is delegated.

### Voting and Consensus Mechanism

After the snapshot process is complete, the system enters the voting phase. This is where your custom NCN logic will have the most impact, as you decide what operators are voting on and how consensus is reached.

#### Initialize Ballot Box

The ballot box is the central account for the voting process. It keeps track of all votes cast by operators and tallies them according to stake weight.

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

Operators can cast votes in the form of a ballot containing their chosen vote data. In the case of the Jito Tip Router, this is represented by a simple `WeatherStatus` in the test, but in a real implementation, this could be a hash of proposed data, transaction, or any other consensus item your NCN requires.

```rust
pub struct Ballot {
    /// The vote data (in the example, weather status)
    vote_data: u8,
    /// Whether the ballot is valid
    is_valid: PodBool,
}
```

When an operator casts a vote, their stake weight is considered, and the vote is tallied in the ballot box. Consensus is reached when votes representing at least 66% of the total stake weight agree on the same ballot.

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

After consensus is reached and a specified waiting period has passed, the accounts for the epoch can be closed to reclaim rent. This is done in the reverse order of creation:

1. Close Ballot Box
2. Close Operator Snapshots
3. Close Epoch Snapshot
4. Close Weight Table
5. Close Epoch State

### Implementing Your Custom NCN Logic

To build your own NCN, you'll need to:

1. Define what operators are voting on (replace the `WeatherStatus` with your own vote data)
2. Determine how consensus results are utilized
3. Build any necessary off-chain infrastructure to support your NCN's use case
4. Implement custom reward distribution logic if needed
