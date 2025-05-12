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

#### `initialize_restaking_and_vault_programs()`

```rust
pub async fn initialize_restaking_and_vault_programs(&mut self) -> TestResult<()> {
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

The function works as follows:

1. **Function Parameters**:
   - `test_ncn`: A mutable reference to the TestNcn structure that tracks the NCN state
   - `operator_count`: The number of operators to create
   - `operator_fees_bps`: Optional fee in basis points (1/100th of a percent) for each operator

2. **Operator Creation Loop**:
   For each operator to be created, the function performs these steps:

   a. **Operator Initialization**:
   ```rust
   let operator_root = restaking_program_client
       .do_initialize_operator(operator_fees_bps)
       .await?;
   ```
   - Creates a new operator account using Jito's restaking program
   - Sets up the operator with the specified fee rate
   - Returns an `operator_root` structure containing the operator's public key and admin keypair

   b. **Bidirectional Handshake Setup**:
   ```rust
   // Initialize NCN-operator state
   restaking_program_client
       .do_initialize_ncn_operator_state(
           &test_ncn.ncn_root,
           &operator_root.operator_pubkey,
       )
       .await?;
   
   // Advance time by one slot
   self.warp_slot_incremental(1).await.unwrap();
   
   // Warm up NCN's connection to operator
   restaking_program_client
       .do_ncn_warmup_operator(&test_ncn.ncn_root, &operator_root.operator_pubkey)
       .await?;
   
   // Warm up operator's connection to NCN
   restaking_program_client
       .do_operator_warmup_ncn(&operator_root, &test_ncn.ncn_root.ncn_pubkey)
       .await?;
   ```
   This establishes a secure bidirectional relationship between the NCN and operator through:
   - Initializing the NCN-operator state
   - Advancing time by one slot to allow state changes to settle
   - Warming up the NCN's connection to the operator
   - Warming up the operator's connection to the NCN

   c. **Operator Registration**:
   ```rust
   test_ncn.operators.push(operator_root);
   ```
   - Adds the newly created operator to the TestNcn structure for tracking

3. **Security Features**:
   - The bidirectional handshake ensures both the NCN and operator acknowledge each other
   - The slot advancement prevents race conditions in state changes
   - The fee parameter allows for operator compensation configuration
   - Each operator gets unique keypairs for secure operations

This function is essential for building the network of operators that will participate in the consensus process. The bidirectional handshake mechanism ensures that only legitimate operators can participate in voting, and the fee structure allows for operator compensation while maintaining network security.

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

The simulation test expects the following outcomes:

1. All operators with delegations should successfully cast votes
2. The system should correctly reach consensus with "Sunny" as the winning status
3. The consensus result should match between the ballot box and consensus result account:
   - Same weather status (Sunny)
   - Same vote weight
4. All accounts should be properly created and cleaned up
5. The consensus result account should persist after cleaning up other accounts, with:
   - Consensus reached flag still set
   - Correct epoch association

These expected outcomes validate the core functionality of the NCN voting system, demonstrating its ability to collect votes, reach consensus, and permanently record the result while efficiently managing on-chain resources.

## Error Cases

While the current simulation test doesn't explicitly test error cases, the NCN system is designed to handle various error conditions:

1. **Invalid vote attempts**: The system verifies operator and admin signatures before allowing votes
2. **Multiple token types**: The system correctly handles tokens with different weights
3. **Various delegation amounts**: From minimal (1 lamport) to very large (10k tokens)
4. **Split votes**: The system correctly identifies the winning vote with majority support
5. **Account management**: Proper creation and cleanup of all necessary accounts

In a comprehensive test suite, additional error cases should be tested, such as:

1. **Zero delegation operators**: Operators with zero delegations attempting to vote
2. **Double voting**: Operators trying to vote more than once in the same epoch
3. **Invalid weather status**: Operators providing an invalid or out-of-range option
4. **Out-of-sequence operations**: Attempting to perform operations out of order
5. **Unauthorized admin actions**: Non-admins attempting to perform privileged operations

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
4. The epoch state tracks the progress of the consensus cycle for this epoch

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
5. The epoch state tracks which stage of the consensus cycle we're in

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
4. This process creates a snapshot of token weights for the current consensus cycle

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
4. This creates a snapshot of token weights that will be used for this consensus cycle

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
