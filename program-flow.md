# NCN Template

This template is meant to be the building blocks for creating and deploying your
own custom NCN using Jito Restaking program.

## System Architecture

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



















### 1. NCN Setup

The process begins with initializing the Network Coordination Node (NCN):

```rust
// Initialize configuration
vault_program_client.do_initialize_config().await?;
restaking_program_client.do_initialize_config().await?;

// Initialize NCN
let ncn_root = restaking_program_client
    .do_initialize_ncn(Some(payer)).await?;

// Set up tip router for this NCN
tip_router_client.setup_tip_router(&ncn_root).await?;
```

### 2. Operator Registration

Multiple operators are registered and connected to the NCN:

```rust
for _ in 0..operator_count {
    // Initialize a new operator
    let operator_root = restaking_program_client
        .do_initialize_operator(operator_fees_bps)
        .await?;

    // Connect NCN and operator
    restaking_program_client
        .do_initialize_ncn_operator_state(
            &ncn_root,
            &operator_root.operator_pubkey,
        )
        .await?;

    // Warmup process to activate the connection
    restaking_program_client
        .do_ncn_warmup_operator(&ncn_root, &operator_root.operator_pubkey)
        .await?;
    restaking_program_client
        .do_operator_warmup_ncn(&operator_root, &ncn_root.ncn_pubkey)
        .await?;
}
```

### 3. Vault Setup

Vaults are created and connected to both the NCN and operators:

```rust
for _ in 0..vault_count {
    // Initialize vault
    let vault_root = vault_program_client
        .do_initialize_vault(
            DEPOSIT_FEE_BPS,
            WITHDRAWAL_FEE_BPS,
            REWARD_FEE_BPS,
            DECIMALS,
            &payer_pubkey,
            Some(token_mint),
        )
        .await?;

    // Connect vault to NCN
    restaking_program_client
        .do_initialize_ncn_vault_ticket(&ncn_root, &vault_root.vault_pubkey)
        .await?;
    restaking_program_client
        .do_warmup_ncn_vault_ticket(&ncn_root, &vault_root.vault_pubkey)
        .await?;

    // Connect vault to NCN (bidirectional connection)
    vault_program_client
        .do_initialize_vault_ncn_ticket(&vault_root, &ncn_root.ncn_pubkey)
        .await?;
    vault_program_client
        .do_warmup_vault_ncn_ticket(&vault_root, &ncn_root.ncn_pubkey)
        .await?;

    // Connect vault to operators
    for operator_root in operators {
        restaking_program_client
            .do_initialize_operator_vault_ticket(
                operator_root,
                &vault_root.vault_pubkey
            )
            .await?;
        restaking_program_client
            .do_warmup_operator_vault_ticket(
                operator_root,
                &vault_root.vault_pubkey
            )
            .await?;
        vault_program_client
            .do_initialize_vault_operator_delegation(
                &vault_root,
                &operator_root.operator_pubkey,
            )
            .await?;
    }
}
```

### 4. Delegations

Stake delegations are added to create the weighted voting system:

```rust
for vault_root in vaults {
    for operator_root in operators {
        vault_program_client
            .do_add_delegation(
                vault_root,
                &operator_root.operator_pubkey,
                delegation_amount,
            )
            .await?;
    }
}
```

### 5. ST Mint Registration

Supported token mints are registered with their respective weights:

```rust
for (mint, weight) in mints {
    tip_router_client
        .do_admin_register_st_mint(ncn, mint.pubkey(), weight)
        .await?;
}

for vault in vaults {
    let vault_pubkey = vault.vault_pubkey;
    let ncn_vault_ticket = NcnVaultTicket::find_program_address(
        &jito_restaking_program::id(),
        &ncn,
        &vault_pubkey
    ).0;

    tip_router_client
        .do_register_vault(ncn, vault_pubkey, ncn_vault_ticket)
        .await?;
}
```

## Per-Epoch Operations

For each epoch, the following operations occur sequentially:

### 1. Initialize Epoch State

A new epoch state account is created to track the current epoch's status:

```rust
tip_router_client
    .do_full_initialize_epoch_state(ncn, epoch)
    .await?;
```

### 2. Set Weight Table

Admin sets the weights for different staked tokens:

```rust
tip_router_client
    .do_full_initialize_weight_table(ncn, epoch)
    .await?;

for entry in vault_registry.st_mint_list {
    if !entry.is_empty() {
        tip_router_client
            .do_admin_set_weight(
                ncn,
                epoch,
                entry.st_mint(),
                entry.weight(),
            )
            .await?;
    }
}
```

### 3. Create Snapshots

Multiple snapshots are taken to capture the state for the current epoch:

```rust
// Initialize epoch snapshot
tip_router_client
    .do_initialize_epoch_snapshot(ncn, epoch)
    .await?;

// Initialize operator snapshots
for operator in operators {
    tip_router_client
        .do_full_initialize_operator_snapshot(operator, ncn, epoch)
        .await?;
}

// Snapshot vault-operator delegations
for operator in operators {
    for vault in vaults {
        // Update vault if needed
        if vault_is_update_needed {
            vault_program_client
                .do_full_vault_update(&vault, &operators)
                .await?;
        }

        tip_router_client
            .do_snapshot_vault_operator_delegation(
                vault,
                operator,
                ncn,
                epoch
            )
            .await?;
    }
}
```

### 4. Voting and Consensus

A ballot box is initialized and operators cast votes:

```rust
// Initialize ballot box
tip_router_client
    .do_full_initialize_ballot_box(ncn, epoch)
    .await?;

// Each operator casts a vote
let weather_status = WeatherStatus::Sunny as u8; // Or other status
for operator_root in operators {
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

// Verify consensus is reached
let ballot_box = tip_router_client.get_ballot_box(ncn, epoch).await?;
assert!(ballot_box.has_winning_ballot());
assert!(ballot_box.is_consensus_reached());
```

The `WeatherStatus` is a simple representation used for voting (Sunny, Cloudy, Rainy). It's a stand-in for the more complex meta merkle root that would be used in production.

### 5. Account Cleanup

After a specified number of epochs, the program cleans up accounts:

```rust
// Wait for the required epochs after consensus
self.warp_epoch_incremental(
    config_account.epochs_after_consensus_before_close() + 1
).await?;

// Close accounts in reverse order of creation
// 1. Close Ballot Box
tip_router_client
    .do_close_epoch_account(ncn, epoch_to_close, ballot_box)
    .await?;

// 2. Close Operator Snapshots
for operator in operators {
    tip_router_client
        .do_close_epoch_account(ncn, epoch_to_close, operator_snapshot)
        .await?;
}

// 3. Close Epoch Snapshot
tip_router_client
    .do_close_epoch_account(ncn, epoch_to_close, epoch_snapshot)
    .await?;

// 4. Close Weight Table
tip_router_client
    .do_close_epoch_account(ncn, epoch_to_close, weight_table)
    .await?;

// 5. Close Epoch State
tip_router_client
    .do_close_epoch_account(ncn, epoch_to_close, epoch_state)
    .await?;
```

## Complete Workflow

The entire process follows this sequence:

1. **Setup Phase**: Initialize NCN, operators, vaults, and establish connections
2. **Configuration**: Set delegations and register token mints
3. **Per-Epoch Operations**:
   - Create epoch state
   - Set weight table
   - Take snapshots (epoch, operators, vault-operator delegations)
   - Initialize ballot box and cast votes
   - Reach consensus on the weather status (representing the meta merkle root)
4. **Cleanup**: Close accounts in reverse order after a specified number of epochs

This flow ensures a decentralized, fair, and transparent process for distributing MEV tips to stakers, with each operator having voting power proportional to their delegated stake.
