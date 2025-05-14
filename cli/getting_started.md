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
   ncn-program-cli admin-fund-account-payer --amount-in-sol 1
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

4. **Setup Epochs and Consensus Cycle**:

   ```bash
   # Create epoch state
   ncn-program-cli create-epoch-state

   # Create weight table
   ncn-program-cli create-weight-table

   # Set epoch weights
   ncn-program-cli set-epoch-weights
   ```

5. **Create Snapshots**:

   ```bash
   # Create epoch snapshot
   ncn-program-cli create-epoch-snapshot

   # Create operator snapshot
   ncn-program-cli create-operator-snapshot --operator <OPERATOR_ADDRESS>

   # Snapshot vault-operator delegations
   ncn-program-cli snapshot-vault-operator-delegation
   ```

6. **Voting Process**:

   ```bash
   # Create ballot box
   ncn-program-cli create-ballot-box

   # Cast vote as an operator
   ncn-program-cli operator-cast-vote
   ```

7. **Query Information**:

   ```bash
   # Get operator snapshot details
   ncn-program-cli get-operator-snapshot

   # Get ballot box results
   ncn-program-cli get-ballot-box
   ```

## Command Groups

The CLI provides the following command categories:

- **Admin Commands**: Configuration and administration
- **Crank Functions**: Update and maintain system state
- **Instructions**: Core program interactions
- **Getters**: Query on-chain state

Refer to `ncn-program-cli --help` for a complete list of available commands.

For detailed usage instructions and examples, refer to the [API documentation](api-docs.md).
