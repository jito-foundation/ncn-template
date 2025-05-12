# NCN Program Documentation

The NCN (Network Consensus Node) Program is a Solana program designed for reaching consensus on weather status in a decentralized network. It manages the collection, voting, and consensus mechanisms across vaults and operators in the ecosystem, leveraging Jito's restaking infrastructure.

## Program Overview

The NCN Program facilitates consensus on weather status through a stake-weighted voting mechanism. It operates in epochs and utilizes a weight-based system to determine the influence of different operators in the consensus process. Consensus is reached when votes representing at least 66% of the total stake weight agree on the same ballot.

## Key Components

1. **Config**: Stores global program configuration including epochs before stall, epochs after consensus before close, and valid slots after consensus
2. **Vault Registry**: Manages registered vaults and supported stake token mints
3. **Weight Table**: Maintains weights for different stake tokens to determine their relative importance
4. **Epoch State**: Tracks epoch-specific state including consensus status and account lifecycle
5. **Ballot Box**: Handles voting on weather status with stake-weighted tallying
6. **Epoch Snapshot**: Captures stake delegations at specific epochs for consistent voting weight
7. **Consensus Result**: Stores the final consensus outcome for each epoch

## Weather Status System

The program uses a simple weather status system as the consensus target:

1. **Sunny (0)**: Clear sunny weather
2. **Cloudy (1)**: Cloudy weather conditions
3. **Rainy (2)**: Rainy weather conditions

Operators vote on these status values, and the program tallies votes based on stake weight to determine the consensus result.

## Consensus Mechanism

The consensus process follows these steps:

1. Operators cast votes with a specific weather status
2. Each vote's influence is weighted by the operator's stake weight
3. Votes are tallied in the ballot box
4. Consensus is reached when a weather status receives ≥66% of the total stake weight
5. The consensus result is recorded with details about the winning status, vote weight, and timing

## Program Instructions

The program supports the following instructions, organized by category:

### Global Management

#### 1. InitializeConfig

Initializes the program configuration with parameters for the consensus mechanism. Requires NCN admin signature.

**Parameters**:
- `epochs_before_stall`: Number of epochs before voting is considered stalled
- `epochs_after_consensus_before_close`: Number of epochs after consensus before accounts can be closed
- `valid_slots_after_consensus`: Number of slots after consensus where voting is still valid

**Accounts**:
1. `config` (writable): The config account PDA to initialize `[seeds = [b"config", ncn.key().as_ref()], bump]`
2. `ncn`: The NCN account this config belongs to
3. `ncn_admin` (signer): Admin authority for the NCN
4. `tie_breaker_admin`: Pubkey of the admin authorized to break voting ties
5. `account_payer` (writable, signer): Account paying for the initialization and rent
6. `system_program`: Solana System Program

**Code Snippet**:
```rust
// Simplified Rust example showing core logic from process_admin_initialize_config
pub fn process_admin_initialize_config(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    epochs_before_stall: u64,
    epochs_after_consensus_before_close: u64,
    valid_slots_after_consensus: u64,
) -> ProgramResult {
    // ... account loading and validation ...
    // ... parameter validation (min/max checks) ...
    // ... admin signature check ...

    // Find PDA and create account
    let (config_pda, config_bump, mut config_seeds) =
        Config::find_program_address(program_id, ncn.key);
    config_seeds.push(vec![config_bump]);

    if config_pda != *config.key {
        return Err(ProgramError::InvalidSeeds);
    }

    AccountPayer::pay_and_create_account(
        program_id,
        ncn.key,
        account_payer,
        config,
        system_program,
        program_id,
        Config::SIZE,
        &config_seeds,
    )?;

    // Initialize the config account data
    let mut config_data = config.try_borrow_mut_data()?;
    config_data[0] = Config::DISCRIMINATOR; // Set discriminator
    let config_account = Config::try_from_slice_unchecked_mut(&mut config_data)?;

    let starting_valid_epoch = Clock::get()?.epoch;

    *config_account = Config::new(
        ncn.key,
        tie_breaker_admin.key,
        starting_valid_epoch,
        valid_slots_after_consensus,
        epochs_before_stall,
        epochs_after_consensus_before_close,
        config_bump,
    );

    Ok(())
}
```

**Client Usage**:
```javascript
// JavaScript example
const tx = new Transaction();
tx.add(
  createInitializeConfigInstruction({
    config: configPDA,
    ncn: ncnAccount,
    ncnAdmin: wallet.publicKey,
    tieBreakerAdmin: tieBreakerAccount,
    accountPayer: wallet.publicKey,
    systemProgram: SystemProgram.programId,
    epochs_before_stall: 10,
    epochs_after_consensus_before_close: 20,
    valid_slots_after_consensus: 1000
  })
);
```

#### 2. InitializeVaultRegistry

Initializes the vault registry for tracking vaults and supported stake token mints.

**Accounts**:
1. `config`: NCN configuration account
2. `vault_registry` (writable): The vault registry account to initialize
3. `ncn`: The NCN account
4. `account_payer` (writable): Account paying for the initialization
5. `system_program`: Solana System Program

**Code Snippet**:
```rust
pub fn process_initialize_vault_registry(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let [ncn_config, vault_registry, ncn, account_payer, system_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Create the vault registry account
    let (vault_registry_pda, vault_registry_bump, mut vault_registry_seeds) =
        VaultRegistry::find_program_address(program_id, ncn.key);
    vault_registry_seeds.push(vec![vault_registry_bump]);

    if vault_registry_pda != *vault_registry.key {
        return Err(ProgramError::InvalidSeeds);
    }

    // Initialize with space for vault entries and mint entries
    AccountPayer::pay_and_create_account(
        program_id,
        ncn.key,
        account_payer,
        vault_registry,
        system_program,
        program_id,
        MAX_REALLOC_BYTES as usize,
        &vault_registry_seeds,
    )?;

    Ok(())
}
```

**Client Usage**:
```javascript
// JavaScript example
const tx = new Transaction();
tx.add(
  createInitializeVaultRegistryInstruction({
    config: configPDA,
    vaultRegistry: vaultRegistryPDA,
    ncn: ncnAccount,
    accountPayer: wallet.publicKey,
    systemProgram: SystemProgram.programId
  })
);
```

#### 3. ReallocVaultRegistry

Resizes the vault registry account to accommodate more vaults.

**Accounts**:
1. `config`: NCN configuration account
2. `vault_registry` (writable): The vault registry account to resize
3. `ncn`: The NCN account
4. `account_payer` (writable): Account paying for the reallocation
5. `system_program`: Solana System Program

**Code Snippet**:
```rust
pub fn process_realloc_vault_registry(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let [config, vault_registry, ncn, account_payer, system_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Perform reallocation
    AccountPayer::pay_and_realloc_account(
        program_id,
        ncn.key,
        account_payer,
        vault_registry,
        system_program,
        MAX_REALLOC_BYTES as usize,
    )?;

    Ok(())
}
```

**Client Usage**:
```javascript
// JavaScript example
const tx = new Transaction();
tx.add(
  createReallocVaultRegistryInstruction({
    config: configPDA,
    vaultRegistry: vaultRegistryPDA,
    ncn: ncnAccount,
    accountPayer: wallet.publicKey,
    systemProgram: SystemProgram.programId
  })
);
```

#### 4. RegisterVault

Registers a vault in the vault registry to participate in the consensus mechanism.

**Accounts**:
1. `config`: NCN configuration account
2. `vault_registry` (writable): The vault registry to update
3. `ncn`: The NCN account
4. `vault`: The vault to register
5. `ncn_vault_ticket`: The connection between NCN and vault from the restaking program

**Code Snippet**:
```rust
pub fn process_register_vault(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let [config, vault_registry, ncn, vault, ncn_vault_ticket] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Load and validate accounts
    Config::load(program_id, config, ncn.key, false)?;
    VaultRegistry::load(program_id, vault_registry, ncn.key, true)?;
    Ncn::load(&jito_restaking_program::id(), ncn, false)?;
    Vault::load(&jito_vault_program::id(), vault, false)?;
    NcnVaultTicket::load(
        &jito_restaking_program::id(),
        ncn_vault_ticket,
        ncn,
        vault,
        false,
    )?;

    // Get current slot and vault details
    let clock = Clock::get()?;
    let slot = clock.slot;

    let mut vault_registry_data = vault_registry.try_borrow_mut_data()?;
    let vault_registry = VaultRegistry::try_from_slice_unchecked_mut(&mut vault_registry_data)?;

    let vault_data = vault.data.borrow();
    let vault_account = Vault::try_from_slice_unchecked(&vault_data)?;

    // Verify the vault's supported mint is registered
    if !vault_registry.has_st_mint(&vault_account.supported_mint) {
        msg!("Supported mint not registered");
        return Err(ProgramError::InvalidAccountData);
    }

    // Register the vault
    vault_registry.register_vault(
        vault.key,
        &vault_account.supported_mint,
        vault_account.vault_index(),
        slot,
    )?;

    Ok(())
}
```

**Client Usage**:
```javascript
// JavaScript example
const tx = new Transaction();
tx.add(
  createRegisterVaultInstruction({
    config: configPDA,
    vaultRegistry: vaultRegistryPDA,
    ncn: ncnAccount,
    vault: vaultAccount,
    ncnVaultTicket: ncnVaultTicketPDA
  })
);
```

### Snapshot Management

#### 5. InitializeEpochState

Initializes the state for a specific epoch, creating a tracking mechanism for that epoch's lifecycle.

**Parameters**:
- `epoch`: The target epoch

**Accounts**:
1. `epoch_marker` (writable): Marker account to prevent duplicate initialization
2. `epoch_state` (writable): The epoch state account to initialize
3. `ncn`: The NCN account
4. `account_payer` (writable): Account paying for initialization
5. `system_program`: Solana System Program

**Code Snippet**:
```rust
pub fn process_initialize_epoch_state(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    epoch: u64,
) -> ProgramResult {
    let [epoch_marker, epoch_state, ncn, account_payer, system_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Validate accounts
    load_system_account(epoch_state, true)?;
    load_system_program(system_program)?;
    Ncn::load(&jito_restaking_program::id(), ncn, false)?;
    AccountPayer::load(program_id, account_payer, ncn.key, true)?;
    
    // Check if the epoch marker already exists (prevents duplicate initialization)
    EpochMarker::check_dne(program_id, epoch_marker, ncn.key, epoch)?;

    // Generate PDA for epoch state
    let (epoch_state_pubkey, epoch_state_bump, mut epoch_state_seeds) =
        EpochState::find_program_address(program_id, ncn.key, epoch);
    epoch_state_seeds.push(vec![epoch_state_bump]);
    
    if epoch_state_pubkey.ne(epoch_state.key) {
        msg!("Incorrect epoch state PDA");
        return Err(ProgramError::InvalidAccountData);
    }

    // Create epoch state account
    AccountPayer::pay_and_create_account(
        program_id,
        ncn.key,
        account_payer,
        epoch_state,
        system_program,
        program_id,
        EpochState::SIZE,
        &epoch_state_seeds,
    )?;

    // Initialize the epoch state data
    let mut epoch_state_data = epoch_state.try_borrow_mut_data()?;
    epoch_state_data[0] = EpochState::DISCRIMINATOR;
    let epoch_state_account = EpochState::try_from_slice_unchecked_mut(&mut epoch_state_data)?;

    epoch_state_account.initialize(ncn.key, epoch, Clock::get()?.slot, epoch_state_bump)?;

    // Create the epoch marker to prevent duplicate initialization
    EpochMarker::initialize(
        program_id,
        ncn.key,
        epoch,
        epoch_marker,
        account_payer,
        system_program,
    )?;

    Ok(())
}
```

**Client Usage**:
```javascript
// JavaScript example
const tx = new Transaction();
tx.add(
  createInitializeEpochStateInstruction({
    epochMarker: epochMarkerPDA,
    epochState: epochStatePDA,
    ncn: ncnAccount,
    accountPayer: wallet.publicKey,
    systemProgram: SystemProgram.programId,
    epoch: currentEpoch
  })
);
```

#### 6. InitializeWeightTable

Initializes the weight table for a specific epoch, which will store the importance weights of different tokens.

**Parameters**:
- `epoch`: The target epoch

**Accounts**:
1. `epoch_marker`: Marker account to prevent duplicate initialization
2. `epoch_state`: The epoch state account for the target epoch
3. `vault_registry`: The vault registry containing registered vaults
4. `ncn`: The NCN account
5. `weight_table` (writable): The weight table account to initialize
6. `account_payer` (writable): Account paying for initialization
7. `system_program`: Solana System Program

**Code Snippet**:
```rust
pub fn process_initialize_weight_table(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    epoch: u64,
) -> ProgramResult {
    let [epoch_marker, epoch_state, vault_registry, ncn, weight_table, account_payer, system_program] =
        accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Validate accounts and epoch state
    EpochState::load_and_check_is_closing(program_id, epoch_state, ncn.key, epoch, false)?;
    VaultRegistry::load(program_id, vault_registry, ncn.key, false)?;
    Ncn::load(&jito_restaking_program::id(), ncn, false)?;
    AccountPayer::load(program_id, account_payer, ncn.key, true)?;
    EpochMarker::check_dne(program_id, epoch_marker, ncn.key, epoch)?;

    // Check systems counts match
    let vault_count = {
        let ncn_data = ncn.data.borrow();
        let ncn = Ncn::try_from_slice_unchecked(&ncn_data)?;
        ncn.vault_count()
    };

    let vault_registry_count = {
        let vault_registry_data = vault_registry.data.borrow();
        let vault_registry = VaultRegistry::try_from_slice_unchecked(&vault_registry_data)?;
        vault_registry.vault_count()
    };

    if vault_count != vault_registry_count {
        msg!("Vault count does not match supported mint count");
        return Err(ProgramError::InvalidAccountData);
    }

    // Create the weight table account
    let (weight_table_pubkey, weight_table_bump, mut weight_table_seeds) =
        WeightTable::find_program_address(program_id, ncn.key, epoch);
    weight_table_seeds.push(vec![weight_table_bump]);

    if weight_table_pubkey.ne(weight_table.key) {
        msg!("Incorrect weight table PDA");
        return Err(ProgramError::InvalidAccountData);
    }

    // Allocate space for the weight table 
    AccountPayer::pay_and_create_account(
        program_id,
        ncn.key,
        account_payer,
        weight_table,
        system_program,
        program_id,
        MAX_REALLOC_BYTES as usize,
        &weight_table_seeds,
    )?;

    Ok(())
}
```

**Client Usage**:
```javascript
// JavaScript example
const tx = new Transaction();
tx.add(
  createInitializeWeightTableInstruction({
    epochMarker: epochMarkerPDA,
    epochState: epochStatePDA,
    vaultRegistry: vaultRegistryPDA,
    ncn: ncnAccount,
    weightTable: weightTablePDA,
    accountPayer: wallet.publicKey,
    systemProgram: SystemProgram.programId,
    epoch: currentEpoch
  })
);
```

#### 7. SetEpochWeights

Sets weights for the epoch using the vault registry data, establishing the relative importance of each token type.

**Parameters**:
- `epoch`: The target epoch

**Accounts**:
1. `epoch_state` (writable): The epoch state account for the target epoch
2. `ncn`: The NCN account
3. `vault_registry`: The vault registry containing registered vaults and mint weights
4. `weight_table` (writable): The weight table to update

**Code Snippet**:
```rust
pub fn process_set_epoch_weights(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    epoch: u64,
) -> ProgramResult {
    let [epoch_state, ncn, vault_registry, weight_table] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Validate accounts
    EpochState::load(program_id, epoch_state, ncn.key, epoch, true)?;
    Ncn::load(&jito_restaking_program::id(), ncn, false)?;
    WeightTable::load(program_id, weight_table, ncn.key, epoch, true)?;
    VaultRegistry::load(program_id, vault_registry, ncn.key, false)?;

    // Check that weight table is initialized but not finalized
    let mut weight_table_data = weight_table.try_borrow_mut_data()?;
    let weight_table_account = WeightTable::try_from_slice_unchecked_mut(&mut weight_table_data)?;
    weight_table_account.check_table_initialized()?;

    if weight_table_account.finalized() {
        msg!("Weight table is finalized");
        return Err(ProgramError::InvalidAccountData);
    }

    // Copy weights from vault registry to weight table
    let mut vault_registry_data = vault_registry.data.borrow_mut();
    let vault_registry_account =
        VaultRegistry::try_from_slice_unchecked_mut(&mut vault_registry_data)?;

    for mint_entry in vault_registry_account.get_valid_mint_entries() {
        let weight_from_mint_entry = mint_entry.weight();
        if weight_from_mint_entry == 0 {
            msg!("Weight is not set");
            return Err(NCNProgramError::WeightNotSet.into());
        }

        // Set weight in the weight table
        weight_table_account.set_weight(
            mint_entry.st_mint(),
            weight_from_mint_entry,
            Clock::get()?.slot,
        )?;
    }

    // Update Epoch State
    {
        let mut epoch_state_data = epoch_state.try_borrow_mut_data()?;
        let epoch_state_account = EpochState::try_from_slice_unchecked_mut(&mut epoch_state_data)?;
        epoch_state_account.update_set_weight(
            weight_table_account.weight_count() as u64,
            weight_table_account.st_mint_count() as u64,
        );
    }

    Ok(())
}
```

**Client Usage**:
```javascript
// JavaScript example
const tx = new Transaction();
tx.add(
  createSetEpochWeightsInstruction({
    epochState: epochStatePDA,
    ncn: ncnAccount,
    vaultRegistry: vaultRegistryPDA,
    weightTable: weightTablePDA,
    epoch: currentEpoch
  })
);
```

#### 8. ReallocWeightTable

Resizes the weight table account to accommodate more entries.

**Parameters**:
- `epoch`: The target epoch

**Accounts**:
1. `epoch_state` (writable): The epoch state account for the target epoch
2. `vault_registry`: The vault registry containing registered vaults
3. `ncn`: The NCN account
4. `weight_table` (writable): The weight table to resize
5. `account_payer` (writable): Account paying for reallocation
6. `system_program`: Solana System Program

**Code Snippet**:
```rust
pub fn process_realloc_weight_table(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    epoch: u64,
) -> ProgramResult {
    let [epoch_state, vault_registry, ncn, weight_table, account_payer, system_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Validate accounts
    EpochState::load(program_id, epoch_state, ncn.key, epoch, true)?;
    VaultRegistry::load(program_id, vault_registry, ncn.key, false)?;
    Ncn::load(&jito_restaking_program::id(), ncn, false)?;
    WeightTable::load_or_expect_dne(program_id, weight_table, ncn.key, epoch, true)?;
    AccountPayer::load(program_id, account_payer, ncn.key, true)?;
    load_system_program(system_program)?;

    // Perform reallocation
    AccountPayer::pay_and_realloc_account(
        program_id,
        ncn.key,
        account_payer,
        weight_table,
        system_program,
        MAX_REALLOC_BYTES as usize,
    )?;

    // If the weight table hasn't been initialized yet, initialize it
    let should_initialize = weight_table.data_len() >= WeightTable::SIZE
        && weight_table.try_borrow_data()?[0] != WeightTable::DISCRIMINATOR;

    if should_initialize {
        let vault_registry_data = vault_registry.data.borrow();
        let vault_registry = VaultRegistry::try_from_slice_unchecked(&vault_registry_data)?;

        let vault_count = vault_registry.vault_count();
        let st_mint_count = vault_registry.st_mint_count();
        let vault_entries = vault_registry.get_vault_entries();
        let mint_entries = vault_registry.get_mint_entries();

        let mut weight_table_data = weight_table.try_borrow_mut_data()?;
        weight_table_data[0] = WeightTable::DISCRIMINATOR;
        let weight_table_account =
            WeightTable::try_from_slice_unchecked_mut(&mut weight_table_data)?;

        // Initialize with data from vault registry
        let (_, weight_table_bump, _) = 
            WeightTable::find_program_address(program_id, ncn.key, epoch);
            
        weight_table_account.initialize(
            ncn.key,
            epoch,
            Clock::get()?.slot,
            vault_count,
            weight_table_bump,
            vault_entries,
            mint_entries,
        )?;

        // Update Epoch State
        {
            let mut epoch_state_data = epoch_state.try_borrow_mut_data()?;
            let epoch_state_account =
                EpochState::try_from_slice_unchecked_mut(&mut epoch_state_data)?;
            epoch_state_account.update_realloc_weight_table(vault_count, st_mint_count as u64);
        }
    }

    Ok(())
}
```

**Client Usage**:
```javascript
// JavaScript example
const tx = new Transaction();
tx.add(
  createReallocWeightTableInstruction({
    epochState: epochStatePDA,
    vaultRegistry: vaultRegistryPDA,
    ncn: ncnAccount,
    weightTable: weightTablePDA,
    accountPayer: wallet.publicKey,
    systemProgram: SystemProgram.programId,
    epoch: currentEpoch
  })
);
```

#### 9. InitializeEpochSnapshot

Initializes the epoch snapshot for storing delegations between vaults and operators.

**Parameters**:
- `epoch`: The target epoch

**Accounts**:
1. `epoch_marker`: Marker account to prevent duplicate initialization
2. `epoch_state` (writable): The epoch state account for the target epoch
3. `config`: NCN configuration account
4. `ncn`: The NCN account
5. `weight_table`: Weight table for the target epoch
6. `epoch_snapshot` (writable): The epoch snapshot account to initialize
7. `account_payer` (writable): Account paying for initialization
8. `system_program`: Solana System Program

**Code Snippet**:
```rust
pub fn process_initialize_epoch_snapshot(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    epoch: u64,
) -> ProgramResult {
    let [epoch_marker, epoch_state, config, ncn, weight_table, epoch_snapshot, account_payer, system_program] =
        accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Validate accounts
    EpochState::load_and_check_is_closing(program_id, epoch_state, ncn.key, epoch, true)?;
    Config::load(program_id, config, ncn.key, false)?;
    Ncn::load(&jito_restaking_program::id(), ncn, false)?;
    AccountPayer::load(program_id, account_payer, ncn.key, true)?;
    EpochMarker::check_dne(program_id, epoch_marker, ncn.key, epoch)?;

    // Verify weight table is finalized
    WeightTable::load(program_id, weight_table, ncn.key, epoch, false)?;
    let vault_count = {
        let weight_table_data = weight_table.data.borrow();
        let weight_table_account = WeightTable::try_from_slice_unchecked(&weight_table_data)?;

        if !weight_table_account.finalized() {
            msg!("Weight table must be finalized before initializing epoch snapshot");
            return Err(NCNProgramError::WeightTableNotFinalized.into());
        }

        weight_table_account.vault_count()
    };

    // Create epoch snapshot account
    let (epoch_snapshot_pubkey, epoch_snapshot_bump, mut epoch_snapshot_seeds) =
        EpochSnapshot::find_program_address(program_id, ncn.key, epoch);
    epoch_snapshot_seeds.push(vec![epoch_snapshot_bump]);

    if epoch_snapshot_pubkey.ne(epoch_snapshot.key) {
        msg!("Incorrect epoch snapshot PDA");
        return Err(ProgramError::InvalidAccountData);
    }

    AccountPayer::pay_and_create_account(
        program_id,
        ncn.key,
        account_payer,
        epoch_snapshot,
        system_program,
        program_id,
        EpochSnapshot::SIZE,
        &epoch_snapshot_seeds,
    )?;

    // Get operator count
    let operator_count: u64 = {
        let ncn_data = ncn.data.borrow();
        let ncn_account = Ncn::try_from_slice_unchecked(&ncn_data)?;
        ncn_account.operator_count()
    };

    if operator_count == 0 {
        msg!("No operators to snapshot");
        return Err(NCNProgramError::NoOperators.into());
    }

    // Initialize epoch snapshot
    let mut epoch_snapshot_data = epoch_snapshot.try_borrow_mut_data()?;
    epoch_snapshot_data[0] = EpochSnapshot::DISCRIMINATOR;
    let epoch_snapshot_account =
        EpochSnapshot::try_from_slice_unchecked_mut(&mut epoch_snapshot_data)?;

    epoch_snapshot_account.initialize(
        ncn.key,
        epoch,
        epoch_snapshot_bump,
        Clock::get()?.slot,
        operator_count,
        vault_count,
    )?;

    // Update epoch state
    {
        let mut epoch_state_data = epoch_state.try_borrow_mut_data()?;
        let epoch_state_account = EpochState::try_from_slice_unchecked_mut(&mut epoch_state_data)?;
        epoch_state_account.update_initialize_epoch_snapshot(operator_count);
    }

    Ok(())
}
```

**Client Usage**:
```javascript
// JavaScript example
const tx = new Transaction();
tx.add(
  createInitializeEpochSnapshotInstruction({
    epochMarker: epochMarkerPDA,
    epochState: epochStatePDA,
    config: configPDA,
    ncn: ncnAccount,
    weightTable: weightTablePDA,
    epochSnapshot: epochSnapshotPDA,
    accountPayer: wallet.publicKey,
    systemProgram: SystemProgram.programId,
    epoch: currentEpoch
  })
);
```

#### 10. InitializeOperatorSnapshot

Initializes a snapshot for a specific operator, storing their stake weights.

**Parameters**:
- `epoch`: The target epoch

**Accounts**:
1. `epoch_marker`: Marker account to prevent duplicate initialization 
2. `epoch_state` (writable): The epoch state account for the target epoch
3. `config`: NCN configuration account
4. `ncn`: The NCN account
5. `operator`: The operator account to snapshot
6. `ncn_operator_ticket`: The connection between NCN and operator
7. `operator_snapshot` (writable): Operator snapshot account to initialize
8. `account_payer` (writable): Account paying for initialization
9. `system_program`: Solana System Program

**Code Snippet**:
```rust
pub fn process_initialize_operator_snapshot(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    epoch: u64,
) -> ProgramResult {
    let [epoch_marker, epoch_state, config, ncn, operator, ncn_operator_ticket, operator_snapshot, account_payer, system_program] =
        accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Validate accounts
    EpochState::load_and_check_is_closing(program_id, epoch_state, ncn.key, epoch, true)?;
    Config::load(program_id, config, ncn.key, false)?;
    Ncn::load(&jito_restaking_program::id(), ncn, false)?;
    Operator::load(&jito_restaking_program::id(), operator, false)?;
    NcnOperatorTicket::load(
        &jito_restaking_program::id(),
        ncn_operator_ticket,
        ncn,
        operator,
        false,
    )?;
    AccountPayer::load(program_id, account_payer, ncn.key, true)?;
    EpochMarker::check_dne(program_id, epoch_marker, ncn.key, epoch)?;

    // Create and initialize operator snapshot
    let (operator_snapshot_pubkey, operator_snapshot_bump, mut operator_snapshot_seeds) =
        OperatorSnapshot::find_program_address(
            program_id,
            operator.key,
            ncn.key,
            epoch,
        );
    operator_snapshot_seeds.push(vec![operator_snapshot_bump]);

    if operator_snapshot_pubkey.ne(operator_snapshot.key) {
        msg!("Incorrect operator snapshot PDA");
        return Err(ProgramError::InvalidAccountData);
    }

    AccountPayer::pay_and_create_account(
        program_id,
        ncn.key,
        account_payer,
        operator_snapshot,
        system_program,
        program_id,
        OperatorSnapshot::SIZE,
        &operator_snapshot_seeds,
    )?;

    // Get NCN operator index and active status
    let (ncn_operator_index, is_active) = {
        let ncn_data = ncn.data.borrow();
        let ncn_account = Ncn::try_from_slice_unchecked(&ncn_data)?;

        let operator_data = operator.data.borrow();
        let operator_account = Operator::try_from_slice_unchecked(&operator_data)?;

        // Find the operator in the NCN's operator list
        let Some(index) = ncn_account
            .operators
            .iter()
            .position(|op| op == operator.key)
        else {
            msg!("Operator not found in NCN");
            return Err(ProgramError::InvalidAccountData);
        };

        (index, operator_account.is_active())
    };

    // Initialize operator snapshot data
    let mut operator_snapshot_data = operator_snapshot.try_borrow_mut_data()?;
    operator_snapshot_data[0] = OperatorSnapshot::DISCRIMINATOR;
    let operator_snapshot_account =
        OperatorSnapshot::try_from_slice_unchecked_mut(&mut operator_snapshot_data)?;

    operator_snapshot_account.initialize(
        operator.key,
        ncn.key,
        epoch,
        operator_snapshot_bump,
        Clock::get()?.slot,
        is_active,
    )?;

    // Update epoch state
    {
        let mut epoch_state_data = epoch_state.try_borrow_mut_data()?;
        let epoch_state_account = EpochState::try_from_slice_unchecked_mut(&mut epoch_state_data)?;
        epoch_state_account.update_realloc_operator_snapshot(ncn_operator_index, is_active)?;
    }

    Ok(())
}
```

**Client Usage**:
```javascript
// JavaScript example
const tx = new Transaction();
tx.add(
  createInitializeOperatorSnapshotInstruction({
    epochMarker: epochMarkerPDA,
    epochState: epochStatePDA,
    config: configPDA,
    ncn: ncnAccount,
    operator: operatorAccount,
    ncnOperatorTicket: ncnOperatorTicketPDA,
    operatorSnapshot: operatorSnapshotPDA,
    accountPayer: wallet.publicKey,
    systemProgram: SystemProgram.programId,
    epoch: currentEpoch
  })
);
```

#### 11. SnapshotVaultOperatorDelegation


Records the delegation between a vault and an operator at a specific epoch.

**Parameters**:
- `epoch`: The target epoch

**Accounts**:
1. `epoch_state` (writable): The epoch state account for the target epoch
2. `ncn`: The NCN account
3. `vault`: The vault account
4. `operator`: The operator account
5. `epoch_snapshot` (writable): Epoch snapshot account
6. `operator_snapshot` (writable): Operator snapshot account
7. `vault_operator_delegation`: The delegation between vault and operator

**Code Snippet**:
```rust
pub fn process_snapshot_vault_operator_delegation(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    epoch: u64,
) -> ProgramResult {
    let [epoch_state, ncn, vault, operator, epoch_snapshot, operator_snapshot, vault_operator_delegation] =
        accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Validate accounts
    EpochState::load(program_id, epoch_state, ncn.key, epoch, true)?;
    Ncn::load(&jito_restaking_program::id(), ncn, false)?;
    Vault::load(&jito_vault_program::id(), vault, false)?;
    Operator::load(&jito_restaking_program::id(), operator, false)?;

    EpochSnapshot::load(program_id, epoch_snapshot, ncn.key, epoch, true)?;
    OperatorSnapshot::load(
        program_id,
        operator_snapshot,
        operator.key,
        ncn.key,
        epoch,
        true,
    )?;
    VaultOperatorDelegation::load(
        &jito_vault_program::id(),
        vault_operator_delegation,
        vault,
        operator,
        false,
    )?;

    // Get delegation stake weight
    let vault_data = vault.data.borrow();
    let vault_account = Vault::try_from_slice_unchecked(&vault_data)?;

    let vault_op_del_data = vault_operator_delegation.data.borrow();
    let vault_op_del_account =
        VaultOperatorDelegation::try_from_slice_unchecked(&vault_op_del_data)?;

    // Get the supported mint weight
    let (st_mint, delegation_pct) = (
        vault_account.supported_mint,
        vault_op_del_account.delegation_pct(),
    );

    // Update epoch snapshot
    {
        let mut epoch_snapshot_data = epoch_snapshot.try_borrow_mut_data()?;
        let epoch_snapshot_account =
            EpochSnapshot::try_from_slice_unchecked_mut(&mut epoch_snapshot_data)?;

        epoch_snapshot_account.note_valid_operator_vault_delegation()?;
    }

    // Update operator snapshot
    {
        let mut operator_snapshot_data = operator_snapshot.try_borrow_mut_data()?;
        let operator_snapshot_account =
            OperatorSnapshot::try_from_slice_unchecked_mut(&mut operator_snapshot_data)?;

        let operator_active = operator_snapshot_account.is_active();
        let finalized = operator_snapshot_account.note_vault_delegation(&st_mint, delegation_pct)?;

        // Update epoch state
        let ncn_operator_index = {
            let ncn_data = ncn.data.borrow();
            let ncn_account = Ncn::try_from_slice_unchecked(&ncn_data)?;

            let Some(index) = ncn_account
                .operators
                .iter()
                .position(|op| op == operator.key)
            else {
                msg!("Operator not found in NCN");
                return Err(ProgramError::InvalidAccountData);
            };

            index
        };

        let mut epoch_state_data = epoch_state.try_borrow_mut_data()?;
        let epoch_state_account = EpochState::try_from_slice_unchecked_mut(&mut epoch_state_data)?;
        epoch_state_account.update_snapshot_vault_operator_delegation(
            ncn_operator_index,
            operator_active && finalized,
        )?;
    }

    Ok(())
}
```

**Client Usage**:
```javascript
// JavaScript example
const tx = new Transaction();
tx.add(
  createSnapshotVaultOperatorDelegationInstruction({
    epochState: epochStatePDA,
    ncn: ncnAccount,
    vault: vaultAccount,
    operator: operatorAccount,
    epochSnapshot: epochSnapshotPDA,
    operatorSnapshot: operatorSnapshotPDA,
    vaultOperatorDelegation: vaultOperatorDelegationPDA,
    epoch: currentEpoch
  })
);
```

#### 12. InitializeBallotBox

Initializes the ballot box for recording and tallying votes on weather status.

**Parameters**:
- `epoch`: The target epoch

**Accounts**:
1. `epoch_marker`: Marker account to prevent duplicate initialization
2. `epoch_state` (writable): The epoch state account for the target epoch
3. `config`: NCN configuration account
4. `ncn`: The NCN account
5. `ballot_box` (writable): The ballot box account to initialize
6. `account_payer` (writable): Account paying for initialization
7. `system_program`: Solana System Program

**Code Snippet**:
```rust
pub fn process_initialize_ballot_box(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    epoch: u64,
) -> ProgramResult {
    let [epoch_marker, epoch_state, config, ncn, ballot_box, account_payer, system_program] =
        accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Validate accounts
    EpochState::load_and_check_is_closing(program_id, epoch_state, ncn.key, epoch, true)?;
    Config::load(program_id, config, ncn.key, false)?;
    Ncn::load(&jito_restaking_program::id(), ncn, false)?;
    AccountPayer::load(program_id, account_payer, ncn.key, true)?;
    EpochMarker::check_dne(program_id, epoch_marker, ncn.key, epoch)?;

    load_system_account(ballot_box, true)?;
    load_system_program(system_program)?;

    // Create ballot box account
    let (ballot_box_pubkey, ballot_box_bump, mut ballot_box_seeds) =
        BallotBox::find_program_address(program_id, ncn.key, epoch);
    ballot_box_seeds.push(vec![ballot_box_bump]);

    if ballot_box_pubkey.ne(ballot_box.key) {
        msg!("Incorrect ballot box PDA");
        return Err(ProgramError::InvalidAccountData);
    }

    msg!(
        "Initializing Ballot Box {} for NCN: {} at epoch: {}",
        ballot_box.key,
        ncn.key,
        epoch
    );
    AccountPayer::pay_and_create_account(
        program_id,
        ncn.key,
        account_payer,
        ballot_box,
        system_program,
        program_id,
        BallotBox::SIZE,
        &ballot_box_seeds,
    )?;

    // Initialize ballot box data
    let mut ballot_box_data = ballot_box.try_borrow_mut_data()?;
    ballot_box_data[0] = BallotBox::DISCRIMINATOR;
    let ballot_box_account = BallotBox::try_from_slice_unchecked_mut(&mut ballot_box_data)?;

    ballot_box_account.initialize(ncn.key, epoch, Clock::get()?.slot, ballot_box_bump)?;

    // Update epoch state
    {
        let mut epoch_state_data = epoch_state.try_borrow_mut_data()?;
        let epoch_state_account = EpochState::try_from_slice_unchecked_mut(&mut epoch_state_data)?;
        epoch_state_account.update_initialize_ballot_box();
    }

    Ok(())
}
```

**Client Usage**:
```javascript
// JavaScript example
const tx = new Transaction();
tx.add(
  createInitializeBallotBoxInstruction({
    epochMarker: epochMarkerPDA,
    epochState: epochStatePDA,
    config: configPDA,
    ncn: ncnAccount,
    ballotBox: ballotBoxPDA,
    accountPayer: wallet.publicKey,
    systemProgram: SystemProgram.programId,
    epoch: currentEpoch
  })
);
```

#### 13. ReallocBallotBox

Resizes the ballot box account to accommodate more votes.

**Parameters**:
- `epoch`: The target epoch

**Accounts**:
1. `epoch_state` (writable): The epoch state account for the target epoch
2. `ballot_box` (writable): The ballot box to resize
3. `ncn`: The NCN account
4. `account_payer` (writable): Account paying for reallocation
5. `system_program`: Solana System Program

**Code Snippet**:
```rust
pub fn process_realloc_ballot_box(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    epoch: u64,
) -> ProgramResult {
    let [epoch_state, ballot_box, ncn, account_payer, system_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Validate accounts and epoch state
    EpochState::load_and_check_is_closing(program_id, epoch_state, ncn.key, epoch, false)?;
    BallotBox::load_or_expect_dne(program_id, ballot_box, ncn.key, epoch, true)?;
    Ncn::load(&jito_restaking_program::id(), ncn, false)?;
    AccountPayer::load(program_id, account_payer, ncn.key, true)?;
    load_system_program(system_program)?;

    // Perform reallocation
    AccountPayer::pay_and_realloc_account(
        program_id,
        ncn.key,
        account_payer,
        ballot_box,
        system_program,
        BallotBox::SIZE,
    )?;

    // If ballot box hasn't been initialized yet, initialize it
    let should_initialize = ballot_box.data_len() >= BallotBox::SIZE
        && ballot_box.try_borrow_data()?[0] != BallotBox::DISCRIMINATOR;

    if should_initialize {
        let (_, ballot_box_bump, _) = 
            BallotBox::find_program_address(program_id, ncn.key, epoch);
            
        let mut ballot_box_data = ballot_box.try_borrow_mut_data()?;
        ballot_box_data[0] = BallotBox::DISCRIMINATOR;
        let ballot_box_account = BallotBox::try_from_slice_unchecked_mut(&mut ballot_box_data)?;

        ballot_box_account.initialize(ncn.key, epoch, Clock::get()?.slot, ballot_box_bump)?;

        // Update Epoch State
        {
            let mut epoch_state_data = epoch_state.try_borrow_mut_data()?;
            let epoch_state_account =
                EpochState::try_from_slice_unchecked_mut(&mut epoch_state_data)?;
            epoch_state_account.update_initialize_ballot_box();
        }
    }

    Ok(())
}
```

**Client Usage**:
```javascript
// JavaScript example
const tx = new Transaction();
tx.add(
  createReallocBallotBoxInstruction({
    epochState: epochStatePDA,
    ballotBox: ballotBoxPDA,
    ncn: ncnAccount,
    accountPayer: wallet.publicKey,
    systemProgram: SystemProgram.programId,
    epoch: currentEpoch
  })
);
```


### Voting System

#### 14. CastVote

Allows an operator to cast a vote on weather status.

**Parameters**:
- `weather_status`: Status code for the vote (0=Sunny, 1=Cloudy, 2=Rainy)
- `epoch`: The target epoch

**Accounts**:
1. `epoch_state` (writable): The epoch state account for the target epoch
2. `config`: NCN configuration account
3. `ballot_box` (writable): The ballot box for recording votes
4. `ncn`: The NCN account
5. `epoch_snapshot`: Epoch snapshot containing stake weights
6. `operator_snapshot`: Operator snapshot containing operator stake
7. `operator`: The operator account casting the vote
8. `operator_voter` (signer): The account authorized to vote on behalf of the operator
9. `consensus_result` (writable): Account for storing the consensus result

**Code Snippet**:
```rust
pub fn process_cast_vote(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    weather_status: u8,
    epoch: u64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let epoch_state = next_account_info(account_info_iter)?;
    let ncn_config = next_account_info(account_info_iter)?;
    let ballot_box = next_account_info(account_info_iter)?;
    let ncn = next_account_info(account_info_iter)?;
    let epoch_snapshot = next_account_info(account_info_iter)?;
    let operator_snapshot = next_account_info(account_info_iter)?;
    let operator = next_account_info(account_info_iter)?;
    let operator_admin = next_account_info(account_info_iter)?;
    let consensus_result = next_account_info(account_info_iter)?;

    // Validate accounts
    load_signer(operator_admin, false)?;
    EpochState::load(program_id, epoch_state, ncn.key, epoch, false)?;
    NcnConfig::load(program_id, ncn_config, ncn.key, false)?;
    Ncn::load(&jito_restaking_program::id(), ncn, false)?;
    Operator::load(&jito_restaking_program::id(), operator, false)?;
    BallotBox::load(program_id, ballot_box, ncn.key, epoch, true)?;
    EpochSnapshot::load(program_id, epoch_snapshot, ncn.key, epoch, false)?;
    OperatorSnapshot::load(
        program_id,
        operator_snapshot,
        operator.key,
        ncn.key,
        epoch,
        false,
    )?;
    ConsensusResult::load(program_id, consensus_result, ncn.key, epoch, true)?;

    // Verify operator voter
    let operator_data = operator.data.borrow();
    let operator_account = Operator::try_from_slice_unchecked(&operator_data)?;
    if *operator_admin.key != operator_account.voter {
        return Err(NCNProgramError::InvalidOperatorVoter.into());
    }

    // Get config parameters
    let valid_slots_after_consensus = {
        let ncn_config_data = ncn_config.data.borrow();
        let ncn_config = NcnConfig::try_from_slice_unchecked(&ncn_config_data)?;
        ncn_config.valid_slots_after_consensus()
    };

    // Get ballot box and verify snapshot is finalized
    let mut ballot_box_data = ballot_box.data.borrow_mut();
    let ballot_box = BallotBox::try_from_slice_unchecked_mut(&mut ballot_box_data)?;

    // Get total stake from epoch snapshot
    let total_stake_weights = {
        let epoch_snapshot_data = epoch_snapshot.data.borrow();
        let epoch_snapshot = EpochSnapshot::try_from_slice_unchecked(&epoch_snapshot_data)?;

        if !epoch_snapshot.finalized() {
            return Err(NCNProgramError::EpochSnapshotNotFinalized.into());
        }

        *epoch_snapshot.stake_weights()
    };

    // Get operator stake from operator snapshot
    let operator_stake_weights = {
        let operator_snapshot_data = operator_snapshot.data.borrow();
        let operator_snapshot =
            OperatorSnapshot::try_from_slice_unchecked(&operator_snapshot_data)?;

        *operator_snapshot.stake_weights()
    };

    // Verify operator has stake
    if operator_stake_weights.stake_weight() == 0 {
        msg!("Operator has zero stake weight, cannot vote");
        return Err(NCNProgramError::CannotVoteWithZeroStake.into());
    }

    let slot = Clock::get()?.slot;
    let ballot = Ballot::new(weather_status);

    // Cast the vote
    ballot_box.cast_vote(
        operator.key,
        &ballot,
        &operator_stake_weights,
        slot,
        valid_slots_after_consensus,
    )?;

    // Tally votes to see if consensus is reached
    ballot_box.tally_votes(total_stake_weights.stake_weight(), slot)?;

    // If consensus is reached, update the consensus result account
    if ballot_box.is_consensus_reached() {
        let winning_ballot_tally = ballot_box.get_winning_ballot_tally()?;
        msg!(
            "Consensus reached for epoch {} with ballot {:?}",
            epoch,
            winning_ballot_tally
        );

        // Update the consensus result account
        let mut consensus_result_data = consensus_result.try_borrow_mut_data()?;
        let consensus_result_account =
            ConsensusResult::try_from_slice_unchecked_mut(&mut consensus_result_data)?;

        consensus_result_account.record_consensus(
            winning_ballot_tally.ballot().weather_status(),
            winning_ballot_tally.stake_weights().stake_weight() as u64,
            total_stake_weights.stake_weight() as u64,
            slot,
            operator.key,
        )?;
    }

    // Update Epoch State
    {
        let mut epoch_state_data = epoch_state.try_borrow_mut_data()?;
        let epoch_state_account = EpochState::try_from_slice_unchecked_mut(&mut epoch_state_data)?;
        epoch_state_account.update_cast_vote(
            ballot_box.operators_voted(),
            ballot_box.is_consensus_reached(),
            slot,
        )?;
    }

    Ok(())
}
```

**Client Usage**:
```javascript
// JavaScript example
const tx = new Transaction();
tx.add(
  createCastVoteInstruction({
    epochState: epochStatePDA,
    config: configPDA,
    ballotBox: ballotBoxPDA,
    ncn: ncnAccount,
    epochSnapshot: epochSnapshotPDA, 
    operatorSnapshot: operatorSnapshotPDA,
    operator: operatorAccount,
    operatorVoter: wallet.publicKey,
    consensusResult: consensusResultPDA,
    weatherStatus: 0, // Sunny weather status
    epoch: currentEpoch
  })
);
```

### Account Management

#### 15. CloseEpochAccount

Closes an epoch-specific account (like `WeightTable`, `EpochSnapshot`, `OperatorSnapshot`, `BallotBox`, or `EpochState` itself) after consensus has been reached and sufficient time has passed (defined by `epochs_after_consensus_before_close` in the `Config`). It reclaims the rent lamports, transferring them to the `account_payer`.

**Parameters**:
- `epoch`: The epoch associated with the account being closed.

**Accounts**:
1. `epoch_marker` (writable): Marker account used to prevent closing already closed/non-existent epoch structures. Will be created if `EpochState` is the `account_to_close`.
2. `epoch_state` (writable): The epoch state account for the target epoch. Must exist and indicate consensus was reached long enough ago.
3. `config`: NCN configuration account (used to check `epochs_after_consensus_before_close`).
4. `ncn`: The NCN account.
5. `account_to_close` (writable): The epoch-specific account to close (e.g., `WeightTable`, `EpochSnapshot`, `OperatorSnapshot`, `BallotBox`, `EpochState`). Must be owned by the NCN program and match the specified epoch.
6. `account_payer` (writable, signer): Account paying for the transaction and receiving the reclaimed rent lamports.
7. `system_program`: Solana System Program (used for creating `epoch_marker` if needed).

**Code Snippet**:
```rust
// Simplified Rust example showing core logic from process_close_epoch_account
pub fn process_close_epoch_account(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    epoch: u64,
) -> ProgramResult {
    // ... account loading and validation ...
    // ... check config for epochs_after_consensus_before_close ...
    // ... check epoch_state shows consensus reached long enough ago ...

    // Determine if the account_to_close is the epoch_state itself
    let closing_epoch_state = account_to_close.key.eq(epoch_state.key);

    // Update epoch state to reflect closing (marks account type as closed)
    {
        let mut epoch_state_data = epoch_state.try_borrow_mut_data()?;
        let epoch_state_account = EpochState::try_from_slice_unchecked_mut(&mut epoch_state_data)?;
        epoch_state_account.set_is_closing();
        // ... logic to mark specific account type closed within epoch_state_account ...
    }

    // Create epoch_marker if closing the epoch_state account
    if closing_epoch_state {
        // ... logic to create and initialize epoch_marker account ...
    }

    // Close the target account and transfer lamports
    AccountPayer::close_account(program_id, account_payer, account_to_close)?;

    Ok(())
}
```

**Client Usage**:
```javascript
// JavaScript example
const tx = new Transaction();
tx.add(
  createCloseEpochAccountInstruction({
    epochState: epochStatePDA,
    config: configPDA,
    ncn: ncnAccount,
    consensusResult: consensusResultPDA,
    accountToClose: accountToClosePDA, // Could be weight_table, ballot_box, etc.
    rentDestination: wallet.publicKey,
    epoch: targetEpoch
  })
);
```

### Admin Controls

#### 16. AdminSetParameters

Updates program configuration parameters after initialization.

**Parameters**:
- `starting_valid_epoch`: Optional starting epoch
- `epochs_before_stall`: Optional number of epochs before stall
- `epochs_after_consensus_before_close`: Optional number of epochs after consensus before close
- `valid_slots_after_consensus`: Optional number of valid slots after consensus

**Accounts**:
1. `config` (writable): NCN configuration account
2. `ncn`: The NCN account
3. `ncn_admin` (signer): Admin authority for the NCN

**Code Snippet**:
```rust
pub fn process_admin_set_parameters(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    starting_valid_epoch: Option<u64>,
    epochs_before_stall: Option<u64>,
    epochs_after_consensus_before_close: Option<u64>,
    valid_slots_after_consensus: Option<u64>,
) -> ProgramResult {
    let [config, ncn, ncn_admin] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Validate accounts
    Config::load(program_id, config, ncn.key, true)?;
    Ncn::load(&jito_restaking_program::id(), ncn, false)?;
    load_signer(ncn_admin, true)?;

    // Verify NCN admin
    let ncn_data = ncn.data.borrow();
    let ncn_account = Ncn::try_from_slice_unchecked(&ncn_data)?;
    if ncn_account.admin != *ncn_admin.key {
        return Err(NCNProgramError::IncorrectNcnAdmin.into());
    }

    // Update config parameters
    let mut config_data = config.try_borrow_mut_data()?;
    let config_account = Config::try_from_slice_unchecked_mut(&mut config_data)?;

    // Validate and update each parameter
    if let Some(starting_valid_epoch) = starting_valid_epoch {
        config_account.set_starting_valid_epoch(starting_valid_epoch)?;
    }

    if let Some(epochs_before_stall) = epochs_before_stall {
        if !(MIN_EPOCHS_BEFORE_STALL..=MAX_EPOCHS_BEFORE_STALL).contains(&epochs_before_stall) {
            return Err(NCNProgramError::InvalidEpochsBeforeStall.into());
        }
        config_account.set_epochs_before_stall(epochs_before_stall);
    }

    if let Some(epochs_after_consensus_before_close) = epochs_after_consensus_before_close {
        if !(MIN_EPOCHS_AFTER_CONSENSUS_BEFORE_CLOSE..=MAX_EPOCHS_AFTER_CONSENSUS_BEFORE_CLOSE)
            .contains(&epochs_after_consensus_before_close)
        {
            return Err(NCNProgramError::InvalidEpochsBeforeClose.into());
        }
        config_account.set_epochs_after_consensus_before_close(epochs_after_consensus_before_close);
    }

    if let Some(valid_slots_after_consensus) = valid_slots_after_consensus {
        if !(MIN_VALID_SLOTS_AFTER_CONSENSUS..=MAX_VALID_SLOTS_AFTER_CONSENSUS)
            .contains(&valid_slots_after_consensus)
        {
            return Err(NCNProgramError::InvalidSlotsAfterConsensus.into());
        }
        config_account.set_valid_slots_after_consensus(valid_slots_after_consensus);
    }

    Ok(())
}
```

**Client Usage**:
```javascript
// JavaScript example
const tx = new Transaction();
tx.add(
  createAdminSetParametersInstruction({
    config: configPDA,
    ncn: ncnAccount,
    ncnAdmin: wallet.publicKey,
    startingValidEpoch: null,
    epochsBeforeStall: 15,
    epochsAfterConsensusBeforeClose: 25,
    validSlotsAfterConsensus: 2000
  })
);
```

#### 17. AdminSetNewAdmin

Sets a new admin for a specific role.

**Parameters**:
- `role`: The admin role to update (currently only TieBreakerAdmin is supported)

**Accounts**:
1. `config` (writable): NCN configuration account
2. `ncn`: The NCN account
3. `ncn_admin` (signer): Current admin authority for the NCN
4. `new_admin`: The new admin address

**Code Snippet**:
```rust
pub fn process_admin_set_new_admin(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    role: ConfigAdminRole,
) -> ProgramResult {
    let [config, ncn_account, ncn_admin, new_admin] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Validate accounts
    load_signer(ncn_admin, true)?;
    NcnConfig::load(program_id, config, ncn_account.key, true)?;
    Ncn::load(&jito_restaking_program::id(), ncn_account, false)?;

    // Verify NCN and admin
    let mut config_data = config.try_borrow_mut_data()?;
    let config = NcnConfig::try_from_slice_unchecked_mut(&mut config_data)?;

    if config.ncn != *ncn_account.key {
        return Err(NCNProgramError::IncorrectNcn.into());
    }

    let ncn_data = ncn_account.data.borrow();
    let ncn = Ncn::try_from_slice_unchecked(&ncn_data)?;

    if ncn.admin != *ncn_admin.key {
        return Err(NCNProgramError::IncorrectNcnAdmin.into());
    }

    // Update admin based on role
    match role {
        ConfigAdminRole::TieBreakerAdmin => {
            config.tie_breaker_admin = *new_admin.key;
            msg!("Tie breaker admin set to {:?}", new_admin.key);
        }
    }

    Ok(())
}
```

**Client Usage**:
```javascript
// JavaScript example
const tx = new Transaction();
tx.add(
  createAdminSetNewAdminInstruction({
    config: configPDA,
    ncn: ncnAccount,
    ncnAdmin: wallet.publicKey,
    newAdmin: newAdminPublicKey,
    role: ConfigAdminRole.TieBreakerAdmin
  })
);
```

#### 18. AdminSetTieBreaker

Allows the tie-breaker admin to resolve stalled votes by selecting a winning ballot.

**Parameters**:
- `weather_status`: Status code for the tie-breaking vote (0=Sunny, 1=Cloudy, 2=Rainy)
- `epoch`: The target epoch

**Accounts**:
1. `epoch_state` (writable): The epoch state account for the target epoch
2. `config`: NCN configuration account
3. `ballot_box` (writable): The ballot box containing votes
4. `ncn`: The NCN account
5. `tie_breaker_admin` (signer): Admin account authorized to break ties

**Code Snippet**:
```rust
pub fn process_admin_set_tie_breaker(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    weather_status: u8,
    epoch: u64,
) -> ProgramResult {
    let [epoch_state, ncn_config, ballot_box, ncn, tie_breaker_admin] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Validate accounts
    EpochState::load(program_id, epoch_state, ncn.key, epoch, true)?;
    NcnConfig::load(program_id, ncn_config, ncn.key, false)?;
    BallotBox::load(program_id, ballot_box, ncn.key, epoch, true)?;
    Ncn::load(&jito_restaking_program::id(), ncn, false)?;
    load_signer(tie_breaker_admin, false)?;

    // Verify tie breaker admin
    let ncn_config_data = ncn_config.data.borrow();
    let ncn_config = NcnConfig::try_from_slice_unchecked(&ncn_config_data)?;

    if ncn_config.tie_breaker_admin.ne(tie_breaker_admin.key) {
        msg!("Tie breaker admin invalid");
        return Err(NCNProgramError::TieBreakerAdminInvalid.into());
    }

    // Set tie breaker in ballot box
    let mut ballot_box_data = ballot_box.data.borrow_mut();
    let ballot_box = BallotBox::try_from_slice_unchecked_mut(&mut ballot_box_data)?;

    let clock = Clock::get()?;
    let current_epoch = clock.epoch;

    ballot_box.set_tie_breaker_ballot(
        weather_status,
        current_epoch,
        ncn_config.epochs_before_stall(),
    )?;

    // Update epoch state
    {
        let mut epoch_state_data = epoch_state.try_borrow_mut_data()?;
        let epoch_state_account = EpochState::try_from_slice_unchecked_mut(&mut epoch_state_data)?;
        epoch_state_account.update_set_tie_breaker_vote(Clock::get()?.slot)?;
    }

    Ok(())
}
```

**Client Usage**:
```javascript
// JavaScript example
const tx = new Transaction();
tx.add(
  createAdminSetTieBreakerInstruction({
    epochState: epochStatePDA,
    config: configPDA,
    ballotBox: ballotBoxPDA,
    ncn: ncnAccount,
    tieBreakerAdmin: wallet.publicKey,
    weatherStatus: 1, // Cloudy weather status
    epoch: currentEpoch
  })
);
```

#### 19. AdminSetWeight

Sets the weight for a specific staked token, determining its influence in voting.

**Parameters**:
- `st_mint`: Public key of the staked token mint
- `weight`: Weight value (importance) of the token
- `epoch`: The target epoch

**Accounts**:
1. `epoch_state` (writable): The epoch state account for the target epoch
2. `ncn`: The NCN account 
3. `weight_table` (writable): The weight table to update
4. `weight_table_admin` (signer): Admin authorized to update weights

**Code Snippet**:
```rust
pub fn process_admin_set_weight(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    st_mint: &Pubkey,
    epoch: u64,
    weight: u128,
) -> ProgramResult {
    let [epoch_state, ncn, weight_table, weight_table_admin] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Validate accounts
    Ncn::load(&jito_restaking_program::id(), ncn, false)?;
    let ncn_weight_table_admin = {
        let ncn_data = ncn.data.borrow();
        let ncn = Ncn::try_from_slice_unchecked(&ncn_data)?;
        ncn.weight_table_admin
    };

    load_signer(weight_table_admin, true)?;
    EpochState::load(program_id, epoch_state, ncn.key, epoch, true)?;
    WeightTable::load(program_id, weight_table, ncn.key, epoch, true)?;

    // Verify weight table admin
    if ncn_weight_table_admin.ne(weight_table_admin.key) {
        msg!("Weight table admin is not correct");
        return Err(NCNProgramError::IncorrectWeightTableAdmin.into());
    }

    // Update weight in weight table
    let mut weight_table_data = weight_table.try_borrow_mut_data()?;
    let weight_table_account = WeightTable::try_from_slice_unchecked_mut(&mut weight_table_data)?;

    weight_table_account.check_table_initialized()?;
    if weight_table_account.finalized() {
        msg!("Weight table is finalized");
        return Err(ProgramError::InvalidAccountData);
    }

    weight_table_account.set_weight(st_mint, weight, Clock::get()?.slot)?;

    // Update epoch state
    {
        let mut epoch_state_data = epoch_state.try_borrow_mut_data()?;
        let epoch_state_account = EpochState::try_from_slice_unchecked_mut(&mut epoch_state_data)?;
        epoch_state_account.update_set_weight(
            weight_table_account.weight_count() as u64,
            weight_table_account.st_mint_count() as u64,
        );
    }

    Ok(())
}
```

**Client Usage**:
```javascript
// JavaScript example
const tx = new Transaction();
tx.add(
  createAdminSetWeightInstruction({
    epochState: epochStatePDA,
    ncn: ncnAccount,
    weightTable: weightTablePDA,
    weightTableAdmin: wallet.publicKey,
    stMint: tokenMintAddress,
    epoch: currentEpoch,
    weight: new BN('1000000000') // 1 billion weight precision
  })
);
```

#### 20. AdminRegisterStMint

Registers a new staked token mint in the vault registry.

**Parameters**:
- `weight`: Optional initial weight for the token

**Accounts**:
1. `config`: NCN configuration account
2. `vault_registry` (writable): The vault registry to update
3. `ncn`: The NCN account
4. `st_mint`: The stake token mint to register
5. `weight_table_admin` (signer): Admin authorized to register tokens

**Code Snippet**:
```rust
pub fn process_admin_register_st_mint(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    weight: Option<u128>,
) -> ProgramResult {
    let [config, vault_registry, ncn, st_mint, weight_table_admin] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Validate accounts
    Config::load(program_id, config, ncn.key, false)?;
    VaultRegistry::load(program_id, vault_registry, ncn.key, true)?;
    Ncn::load(&jito_restaking_program::id(), ncn, false)?;
    load_signer(weight_table_admin, false)?;

    // Verify weight table admin
    let ncn_data = ncn.data.borrow();
    let ncn_account = Ncn::try_from_slice_unchecked(&ncn_data)?;
    if ncn_account.weight_table_admin != *weight_table_admin.key {
        return Err(NCNProgramError::IncorrectWeightTableAdmin.into());
    }

    // Register the stake token mint
    let mut vault_registry_data = vault_registry.try_borrow_mut_data()?;
    let vault_registry_account =
        VaultRegistry::try_from_slice_unchecked_mut(&mut vault_registry_data)?;

    vault_registry_account.register_st_mint(st_mint.key, weight)?;

    Ok(())
}
```

**Client Usage**:
```javascript
// JavaScript example
const tx = new Transaction();
tx.add(
  createAdminRegisterStMintInstruction({
    config: configPDA,
    vaultRegistry: vaultRegistryPDA,
    ncn: ncnAccount,
    stMint: tokenMintAddress,
    weightTableAdmin: wallet.publicKey,
    weight: new BN('1000000000') // Optional initial weight
  })
);
```

#### 21. AdminSetStMint

Updates an existing staked token mint in the vault registry.

**Parameters**:
- `st_mint`: Public key of the staked token mint
- `weight`: Optional new weight for the token

**Accounts**:
1. `config`: NCN configuration account
2. `vault_registry` (writable): The vault registry to update
3. `ncn`: The NCN account
4. `weight_table_admin` (signer): Admin authorized to update token weights

**Code Snippet**:
```rust
pub fn process_admin_set_st_mint(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    st_mint: &Pubkey,
    weight: Option<u128>,
) -> ProgramResult {
    let [config, vault_registry, ncn, weight_table_admin] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Validate accounts
    Config::load(program_id, config, ncn.key, false)?;
    VaultRegistry::load(program_id, vault_registry, ncn.key, true)?;
    Ncn::load(&jito_restaking_program::id(), ncn, false)?;
    load_signer(weight_table_admin, false)?;

    // Verify weight table admin
    let ncn_data = ncn.data.borrow();
    let ncn_account = Ncn::try_from_slice_unchecked(&ncn_data)?;
    if ncn_account.weight_table_admin != *weight_table_admin.key {
        return Err(NCNProgramError::IncorrectWeightTableAdmin.into());
    }

    // Update the stake token mint
    let mut vault_registry_data = vault_registry.try_borrow_mut_data()?;
    let vault_registry_account =
        VaultRegistry::try_from_slice_unchecked_mut(&mut vault_registry_data)?;

    if let Some(weight) = weight {
        vault_registry_account.set_st_mint_weight(st_mint, weight)?;
    }

    Ok(())
}
```

**Client Usage**:
```javascript
// JavaScript example
const tx = new Transaction();
tx.add(
  createAdminSetStMintInstruction({
    config: configPDA,
    vaultRegistry: vaultRegistryPDA,
    ncn: ncnAccount,
    weightTableAdmin: wallet.publicKey,
    stMint: tokenMintAddress,
    weight: new BN('2000000000') // Optional new weight
  })
);
```

## Program Flow

1. **Initialization**: The program is initialized with a configuration, vault registry, and other foundational accounts.
2. **Epoch Setup**: For each epoch, an epoch state and weight table are initialized.
3. **Snapshot Creation**: Operator and vault delegations are recorded in snapshots to establish voting weights.
4. **Voting**: Operators cast votes on weather status with influence based on their stake weight.
5. **Consensus Determination**: When votes for a status reach ≥66% of total stake weight, consensus is achieved.
6. **Result Recording**: The consensus result is stored with the winning status, vote weights, and consensus slot.
7. **Account Cleanup**: After sufficient time has passed, epoch accounts can be closed to reclaim rent.

## Customization Potential

While this implementation uses weather status as the consensus target, the NCN framework can be customized for various consensus applications:

1. Replace the weather status with other vote data (hashes, transaction approval, etc.)
2. Modify consensus thresholds (currently fixed at 66%)
3. Adjust epoch and timing parameters
4. Implement custom reward distribution logic

## Error Handling

The program defines various error conditions including:
- Arithmetic errors (overflow, underflow)
- Registry and table errors (duplicates, capacity limits)
- Voting errors (duplicate votes, invalid voters, zero stake)
- Administrative errors (incorrect permissions)
- Timing errors (epoch constraints, account lifecycle)
- Consensus errors (consensus not reached, already reached)

## Usage Notes

- The program operates in epochs and requires proper initialization of epoch-specific accounts.
- Weight tables determine the importance of different staked tokens in the voting process.
- Operators must have non-zero stake weight to cast votes.
- Consensus requires ≥66% of total stake weight to agree on a weather status.
- Tie-breaker admins can resolve stalled votes after a configurable period.
- Accounts follow a lifecycle and can be closed after consensus and sufficient time has passed.
- The program supports up to 64 vaults and stake tokens, and 256 operators. 