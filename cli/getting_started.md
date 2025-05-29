# NCN Program CLI

## Official Accounts

| Account             | Address                                      |
| ------------------- | -------------------------------------------- |
| Test NCN Program ID | 7rNw1g2ZUCdTrCyVGZwCJLnbp3ssTRK5mdkH8gm9AKE8 |
| Test NCN            | 5zqy3uyXMi5Uurup7S5kKUUuwHGnGcykVwwUik37fm6i |

## Setup CLIs

Install the NCN Program CLI

```bash
cargo build --release
cargo install --path ./cli --bin ncn-program-cli --locked
```

Ensure it has been installed

```bash
ncn-program-cli --help
```

Clone and Install the Restaking and Vault CLI in a different directory

```bash
cd ..
git clone https://github.com/jito-foundation/restaking.git
cd restaking
cargo build --release
cargo install --path ./cli --bin jito-restaking-cli
```

Ensure it works

```bash
jito-restaking-cli --help
```

## CLI Configuration

The CLI supports the following configuration options:

```bash
# Set RPC URL (defaults to Mainnet)
export RPC_URL="https://api.devnet.solana.com"

# Set commitment level
export COMMITMENT="confirmed"

# Set NCN Program ID (if different from default)
export NCN_PROGRAM_ID="7rNw1g2ZUCdTrCyVGZwCJLnbp3ssTRK5mdkH8gm9AKE8"


# Set Jito Restaking and Vault Program IDs
export RESTAKING_PROGRAM_ID=RestkWeAVL8fRGgzhfeoqFhsqKRchg6aa1XrcH96z4Q
export VAULT_PROGRAM_ID=Vau1t6sLNxnzB7ZDsef8TLbPLfyZMYXH8WTNqUdm9g8

# Set keypair path
export KEYPAIR_PATH="~/.config/solana/id.json"

# Set NCN address
export NCN="5zqy3uyXMi5Uurup7S5kKUUuwHGnGcykVwwUik37fm6i"
```

Or you can set them using a .env file, refer to .env.example to learn more

## Basic Usage Flow

Setting up and using the NCN program follows this general workflow:

1. **Setup Prerequisites**:

   ```bash
   # Fund your payer account if needed
   ncn-program-cli admin-fund-account-payer --amount-in-sol 10
   ```

2. **Initialize the NCN Program**:

   ```bash
   # Create program configuration with tie-breaker admin
   ncn-program-cli admin-create-config --tie-breaker-admin <ADMIN_ADDRESS>

   # Create the vault registry
   ncn-program-cli create-vault-registry
   ```

3. **Register Tokens and Vaults**:

   ```bash
   # Register a stake token mint with specified weight
   ncn-program-cli admin-register-st-mint --vault <VAULT_ADDRESS> --weight <WEIGHT> --keypair-path <NCN_ADMIN_KEYPAIR_PATH>

   # Register vaults
   ncn-program-cli crank-register-vaults
   ```

4. Running the keeper command to automate epoch management:

   ```bash
   ncn-program-cli keeper --loop-timeout-ms 600000 --error-timeout-ms 10000 --cluster testnet
   ```

Keeper command will run these commands internally:

1. **Setup Epochs and Consensus Cycle**:

   ```bash
   # Create epoch state
   ncn-program-cli create-epoch-state

   # Create weight table
   ncn-program-cli create-weight-table

   # Set epoch weights
   ncn-program-cli set-epoch-weights
   ```

1. **Create Snapshots**:

   ```bash
   # Create epoch snapshot
   ncn-program-cli create-epoch-snapshot

   # Create operator snapshot
   ncn-program-cli create-operator-snapshot --operator <OPERATOR_ADDRESS>

   # Snapshot vault-operator delegations
   ncn-program-cli snapshot-vault-operator-delegation
   ```

1. **Voting Process**:

   ```bash
   # Create ballot box
   ncn-program-cli create-ballot-box

   # or
   ncn-program-cli crank-vote
   ```

## Command Groups

The CLI provides the following command categories:

- **Admin Commands**: Configuration and administration
- **Crank Functions**: Update and maintain system state
- **Getters**: Query on-chain state
- **Instructions**: Core program interactions
- **Keeper Command**: Automated epoch management

Refer to `ncn-program-cli --help` for a complete list of available commands.

### Keeper Command

The `keeper` command is responsible for automating various epoch-related tasks, such as creating epoch states, snapshotting, and managing votes. It runs as a continuous process, monitoring the chain and executing necessary actions based on the current epoch state.

**Example Usage:**

```bash
ncn-program-cli keeper \
  --loop-timeout-ms 600000 \
  --cluster testnet \
  --region local
```

This command starts the keeper process with a loop timeout of 10 minutes, an error timeout of 10 seconds, targeting the testnet cluster, and identifying the region as local for metrics.

For detailed usage instructions and examples, refer to the [API documentation](api-docs.md).
