# NCN Program Template

## Program Summary

The NCN (Network Consensus Node) Program is a Solana program designed for reaching consensus on weather status in a decentralized network. It manages the collection, voting, and consensus mechanisms across vaults and operators in the ecosystem, leveraging Jito's restaking infrastructure.

Key features:

- Stake-weighted voting mechanism (66% consensus threshold)
- Epoch-based consensus cycles
- Support for multiple stake token mints with configurable weights
- Weather status system (Sunny, Cloudy, Rainy)
- Admin controls for configuration and tie-breaking

## Testing Setup

### Prerequisites

1. Build the ncn program: `cargo build-sbf --manifest-path program/Cargo.toml --sbf-out-dir integration_tests/tests/fixtures`
2. Run tests: `SBF_OUT_DIR=integration_tests/tests/fixtures cargo test`

## Usage Flow

1. **Initialize** the program with configuration, vault registry, and core accounts
2. **Setup Epochs** by creating epoch state and weight tables for each consensus period
3. **Create Snapshots** of operators and vaults to establish voting weights
4. **Cast Votes** on weather status with influence based on stake weight
5. **Achieve Consensus** when votes for a status reach ≥66% of total stake weight
6. **Record Results** with the winning status, voting statistics, and timing data
7. **Clean Up** accounts after sufficient time has passed to reclaim rent

## Customization

While this implementation uses weather status as the consensus target, the framework can be adapted for various applications:

- Replace weather status with other vote data
- Modify consensus thresholds
- Adjust epoch and timing parameters
- Implement custom reward distribution logic

## Deploy

- build .so file: `cargo-build-sbf`

- create a new keypair: `solana-keygen new -o target/tmp/buffer.json`

- Deploy: `solana program deploy --use-rpc --buffer target/tmp/buffer.json --with-compute-unit-price 10000 --max-sign-attempts 10000 target/deploy/ncn_program.so`

## Upgrade

- (Pre Upgrade) Write to buffer: `solana program write-buffer --use-rpc --buffer target/tmp/buffer.json --with-compute-unit-price 10000 --max-sign-attempts 10000 target/deploy/ncn_program.so`

- Upgrade: `solana program upgrade $(solana address --keypair target/tmp/buffer.json) $(solana address --keypair target/deploy/ncn_program-keypair.json)`

- Close Buffers: `solana program close --buffers`

- Upgrade Program Size: `solana program extend $(solana address --keypair target/deploy/ncn_program_program-keypair.json) 100000`

## More info

You can check the docs for more into [here](TODO: add link). There is also a tutorial on how to use the program [here](TODO: add link).
