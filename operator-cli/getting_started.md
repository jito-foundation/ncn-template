# NCN Operator CLI

## Official Accounts

| Account             | Address                                      |
| ------------------- | -------------------------------------------- |
| Test NCN Program ID | 7rNw1g2ZUCdTrCyVGZwCJLnbp3ssTRK5mdkH8gm9AKE8 |
| Test NCN            | 5zqy3uyXMi5Uurup7S5kKUUuwHGnGcykVwwUik37fm6i |

## Setup CLIs

Install the NCN Operator CLI

```bash
cargo build --release
cargo install --path ./cli --bin ncn-operator-cli --locked
```

Ensure it has been installed

```bash
ncn-operator-cli --help
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

You can setup a .env file to store your configuration variables

```bash
cp .env.example .env
```

Edit the .env file with your own values for the following variables:


- RPC_URL: The RPC URL for the Solana cluster you want to use
- COMMITMENT: The commitment level for the Solana cluster you want to use
- NCN_PROGRAM_ID: The NCN Program ID
- RESTAKING_PROGRAM_ID: The Restaking Program ID
- VAULT_PROGRAM_ID: The Vault Program ID
- KEYPAIR_PATH: The path to the keypair file for the payer account
- NCN: The NCN address
- OPERATOR: The operator address
- OPENWEATHER_API_KEY: The Open Weather API key

or you can set them using the following commands:

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

# Set operator address
export OPERATOR="BSia35bXHZx69XzCQeMUnWqZJsUwJURVvuUg8Jup2BcP"

# Set open weather API key
export OPENWEATHER_API_KEY="a40a2afb0c98202f983b52b6a9719f0f"
```

## Basic Usage Flow

Setting up and using the NCN program follows this general workflow:

1. **Running the keeper**:

   ```bash
   # Run the keeper
   ncn-operator-cli keeper --loop-timeout-ms 600000 --error-timeout-ms 10000 --cluster testnet
   ```

2. **Vote for a specific epoch**:

   ```bash
   # Vote for a specific epoch
   ncn-operator-cli operator-cast-vote --epoch 888
   ```

## Command Groups

The CLI provides the following command categories:

- **Keeper Command**: Automated epoch management
- **Instructions**: Core program interactions
- **Getters**: Query on-chain state

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

### Instructions

The only instruction command is the `operator-cast-vote` command.

**Example Usage:**

```bash
ncn-operator-cli operator-cast-vote --epoch 888
```

### Getters

You have the following getters:

- `get-ncn` — Get the NCN address
- `get-ncn-operator-state` — Get the NCN operator state
- `get-vault-ncn-ticket` — Get the vault NCN ticket
- `get-ncn-vault-ticket` — Get the NCN vault ticket
- `get-vault-operator-delegation` — Get the vault operator delegation
- `get-all-tickets` — Get all tickets
- `get-all-operators-in-ncn` — Get all operators in the NCN
- `get-all-vaults-in-ncn` — Get all vaults in the NCN
- `get-ncn-program-config` — Get the NCN program config
- `get-vault-registry` — Get the vault registry
- `get-weight-table` — Get the weight table
- `get-epoch-state` — Get the epoch state
- `get-epoch-snapshot` — Get the epoch snapshot
- `get-operator-snapshot` — Get the operator snapshot
- `get-ballot-box` — Get the ballot box
- `get-account-payer` — Get the account payer
- `get-total-epoch-rent-cost` — Get the total epoch rent cost
- `get-consensus-result` — Get the consensus result
- `get-operator-stakes` — Get the operator stakes
- `get-vault-stakes` — Get the vault stakes
- `get-vault-operator-stakes` — Get the vault operator stakes

For detailed usage instructions and examples, refer to the [API documentation](api-docs.md).


