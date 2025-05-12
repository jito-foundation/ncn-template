# NCN Program Template Code Review Guide

## Table of Contents
1. [Project Overview](#project-overview)
2. [Proposed Way for Reviewing the Code](#proposed-way-for-reviewing-the-code)
   - [Files to Review in Order of Execution](#files-to-review-in-order-of-execution-based-on-simulation_testsrs)
   - [Reviewing the Simulation Test](#reviewing-the-simulation-test)
   - [Reviewing the CLI Code](#reviewing-the-cli-code)
3. [Repository Structure](#repository-structure)
4. [Key Concepts](#key-concepts)
5. [Voting Flow](#voting-flow-from-simulation_testsrs)
6. [Areas of Focus for Review](#areas-of-focus-for-review)
7. [Steps to Run Tests](#steps-to-run-tests)
8. [Dependencies](#dependencies)
9. [Specific Questions for Review](#specific-questions-for-review)

## Project Overview
The NCN Program Template is a Solana program that implements a Network Consensus Node (NCN) voting mechanism for weather status consensus. The system allows operators to cast votes weighted by their stake delegations from various vaults, with different token mints having different voting weights.

## Proposed Way for Reviewing the Code

The recommended approach for reviewing this codebase is to start with the onchain program code, which is located in the `/programs` and `/core` directories. This approach allows reviewers to understand the core functionality before diving into the integration tests and other components.

### Files to Review in Order of Execution (Based on simulation_test.rs)

1. **Core Module Files**:
   - `/core/src/config.rs` - Program configuration
   - `/core/src/constants.rs` - Program constants including base weight values
   - `/core/src/error.rs` - Error definitions
   - `/core/src/account_payer.rs` - Account payment logic

2. **Program Module Files** (in order of execution):
   - `/programs/ncn-program/src/instructions/admin_initialize_config.rs` - Initializes program configuration
   - `/programs/ncn-program/src/instructions/initialize_vault_registry.rs` - Sets up vault tracking
   - `/programs/ncn-program/src/instructions/admin_register_st_mint.rs` - Registers supported token mints
   - `/programs/ncn-program/src/instructions/register_vault.rs` - Registers vaults with the program
   - `/programs/ncn-program/src/instructions/initialize_epoch_state.rs` - Creates epoch state
   - `/programs/ncn-program/src/instructions/initialize_weight_table.rs` - Creates weight tables for voting calculation
   - `/programs/ncn-program/src/instructions/set_epoch_weights.rs` - Updates weights for current epoch
   - `/programs/ncn-program/src/instructions/initialize_epoch_snapshot.rs` - Records current epoch state
   - `/programs/ncn-program/src/instructions/initialize_operator_snapshot.rs` - Records operator state
   - `/programs/ncn-program/src/instructions/snapshot_vault_operator_delegation.rs` - Records delegations
   - `/programs/ncn-program/src/instructions/initialize_ballot_box.rs` - Sets up the ballot box for voting
   - `/programs/ncn-program/src/instructions/cast_vote.rs` - Records operator votes
   - `/programs/ncn-program/src/instructions/close_epoch_account.rs` - Cleans up epoch-related accounts

3. **Core Data Structure Files** (referenced throughout execution):
   - `/core/src/ballot_box.rs` - Manages voting and tallying mechanisms
   - `/core/src/consensus_result.rs` - Stores the final consensus outcome
   - `/core/src/epoch_snapshot.rs` - Records state at specific epochs
   - `/core/src/epoch_state.rs` - Manages the per-epoch state
   - `/core/src/vault_registry.rs` - Tracks vaults participating in the NCN
   - `/core/src/weight_table.rs` - Manages token weights for voting
   - `/core/src/stake_weight.rs` - Calculates voting power based on stake

This order follows the execution flow in the simulation tests, starting with initialization and configuration, followed by the main voting process, and ending with cleanup operations.

### Reviewing the Simulation Test

After understanding the core program files, the next step is to review the simulation test itself and its imported files. The simulation test (`integration_tests/tests/ncn_program/simulation_test.rs`) provides an end-to-end test of the system and demonstrates the complete flow of the program.

#### Key Files to Review:

1. **Test Framework Files**:
   - `integration_tests/tests/fixtures/test_builder.rs` - Creates the test environment
   - `integration_tests/tests/fixtures/test_result.rs` - Handles test results

2. **Client Files**:
   - `integration_tests/tests/fixtures/ncn_program_client.rs` - Client for interacting with the NCN program
   - `integration_tests/tests/fixtures/vault_client.rs` - Client for interacting with the vault program
   - `integration_tests/tests/fixtures/restaking_program_client.rs` - Client for interacting with the restaking program


The simulation test demonstrates:
- Setting up the test environment
- Initializing the NCN, operators, and vaults
- Registering support tokens with weights
- Delegating stakes from vaults to operators
- Running the voting process
- Verifying the consensus results
- Cleaning up epoch accounts

Reviewing these files will help understand how the program is expected to be used in a real-world scenario.

### Reviewing the CLI Code

The CLI (`/cli` directory) provides a command-line interface for interacting with the NCN program. This is the primary tool for operators and administrators to interact with the program in production.

#### Key Files to Review:

1. **Main CLI Files**:
   - `/cli/src/main.rs` - Entry point for the CLI application
   - `/cli/src/commands.rs` - Defines all available CLI commands
   - `/cli/src/config.rs` - CLI configuration and settings

2. **Command Implementation Files**:
   - `/cli/src/commands/admin.rs` - Administrative commands (initialize config, register mints, etc.)
   - `/cli/src/commands/vault.rs` - Vault management commands
   - `/cli/src/commands/vote.rs` - Voting-related commands
   - `/cli/src/commands/query.rs` - Query commands for retrieving program state

3. **Client and Utility Files**:
   - `/cli/src/client.rs` - Client for interacting with the NCN program
   - `/cli/src/utils.rs` - Utility functions for the CLI

The CLI provides a user-friendly interface for:
- Initializing and configuring the program
- Registering and managing vaults
- Casting votes and managing the voting process
- Querying program state and results

Reviewing the CLI code helps understand the intended user workflows and how operators will interact with the program in production.

## Repository Structure

### Core Module (`ncn-program-core`)
Contains the core data structures and business logic:

1. **Data Structures**:
   - `ballot_box.rs` - Manages voting and tallying mechanisms
   - `consensus_result.rs` - Stores the final consensus outcome
   - `epoch_snapshot.rs` - Records state at specific epochs
   - `epoch_state.rs` - Manages the per-epoch state
   - `vault_registry.rs` - Tracks vaults participating in the NCN
   - `weight_table.rs` - Manages token weights for voting
   - `stake_weight.rs` - Calculates voting power based on stake

2. **System Configuration**:
   - `config.rs` - Program configuration
   - `constants.rs` - Program constants including base weight values
   - `error.rs` - Error definitions
   - `account_payer.rs` - Account payment logic

### Program Module (`ncn-program`)
Implements the Solana program instructions:

1. **Administrative Instructions**:
   - `admin_initialize_config.rs` - Initializes program configuration
   - `admin_register_st_mint.rs` - Registers supported token mints
   - `admin_set_st_mint.rs` - Updates token mint parameters
   - `admin_set_weight.rs` - Configures voting weights
   - `admin_set_parameters.rs` - Updates program parameters
   - `admin_set_new_admin.rs` - Changes program admin

2. **Voting System Instructions**:
   - `initialize_ballot_box.rs` - Sets up the ballot box for voting
   - `initialize_weight_table.rs` - Creates weight tables for voting calculation
   - `cast_vote.rs` - Records operator votes
   - `set_epoch_weights.rs` - Updates weights for current epoch

3. **Account Management**:
   - `initialize_vault_registry.rs` - Sets up vault tracking
   - `register_vault.rs` - Registers vaults with the program
   - `initialize_epoch_state.rs` - Creates epoch state
   - `initialize_epoch_snapshot.rs` - Records current epoch state
   - `initialize_operator_snapshot.rs` - Records operator state
   - `snapshot_vault_operator_delegation.rs` - Records delegations
   - `close_epoch_account.rs` - Cleans up epoch-related accounts

4. **Memory Management**:
   - `realloc_ballot_box.rs`, `realloc_vault_registry.rs`, `realloc_weight_table.rs` - Resize accounts as needed

## Key Concepts

1. **Network Consensus Node (NCN)**: Central entity that manages the consensus process
2. **Operators**: Validators that participate in voting
3. **Vaults**: Hold delegated tokens that contribute to voting weight
4. **Support Tokens (ST)**: Different token mints with configurable weights
5. **Epoch-based Voting**: Consensus happens in epochs with snapshots taken at the beginning

## Voting Flow (from `simulation_test.rs`)

1. **Setup & Initialization**:
   - Initialize NCN, operators, and vaults
   - Register support tokens with weights
   - Link vaults to operators through delegations

2. **Per-epoch Voting Process**:
   - Initialize epoch state and weight tables
   - Take snapshots of current delegations and weights
   - Initialize ballot box
   - Operators cast votes with weight proportional to delegations
   - System determines consensus based on weighted voting
   - Results are stored in consensus_result account

3. **Cleanup**:
   - Close epoch accounts while preserving consensus results

## Areas of Focus for Review

1. **Security**: Look for proper access controls on admin functions and vote validation
2. **Weight Calculation**: Ensure stake weights are calculated correctly
3. **Consensus Logic**: Verify the accuracy of consensus determination
4. **Account Management**: Check account lifecycle (creation, reallocation, closing)
5. **Error Handling**: Examine error cases and edge conditions

## Steps to Run Tests
1. Check versions (ensure Solana version 2.2.6 and Cargo 1.81)
2. Run `./scripts/setup-test-ledger.sh`
3. Build the program:
   ```
   cargo build-sbf --manifest-path program/Cargo.toml --sbf-out-dir integration_tests/tests/fixtures
   ```
4. Run tests:
   ```
   RUST_LOG=error SBF_OUT_DIR=integration_tests/tests/fixtures cargo test --no-fail-fast
   ```

## Dependencies
- Jito Restaking and Vault programs
- Solana Program Framework
- SPL Token

## Specific Questions for Review
1. Is the weight calculation mechanism fair and resistant to manipulation?
2. Are administrative operations properly secured?
3. Can the voting system handle edge cases (ties, no consensus, etc.)?
4. Is account management properly handled for all resources?
5. Does the system scale effectively with many operators and vaults? 