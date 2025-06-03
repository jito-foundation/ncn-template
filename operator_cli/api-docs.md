---
title: CLI
category: Jekyll
layout: post
weight: 1
---

# Command-Line Help for `ncn-operator-cli`

This document contains the help content for the `ncn-operator-cli` command-line program.

## `ncn-operator-cli`

A CLI for creating and managing the ncn program

**Usage:** `ncn-operator-cli [OPTIONS] <COMMAND>`

###### **Subcommands:**

- `keeper` — Keeper
- `operator-cast-vote` — Instructions
- `get-ncn` — Getters
- `get-ncn-operator-state` —
- `get-vault-ncn-ticket` —
- `get-ncn-vault-ticket` —
- `get-vault-operator-delegation` —
- `get-all-tickets` —
- `get-all-operators-in-ncn` —
- `get-all-vaults-in-ncn` —
- `get-ncn-program-config` —
- `get-vault-registry` —
- `get-weight-table` —
- `get-epoch-state` —
- `get-epoch-snapshot` —
- `get-operator-snapshot` —
- `get-ballot-box` —
- `get-account-payer` —
- `get-total-epoch-rent-cost` —
- `get-consensus-result` —
- `get-operator-stakes` —
- `get-vault-stakes` —
- `get-vault-operator-stakes` —

###### **Options:**

- `--rpc-url <RPC_URL>` — RPC URL to use

  Default value: `https://api.mainnet-beta.solana.com`

- `--commitment <COMMITMENT>` — Commitment level

  Default value: `confirmed`

- `--priority-fee-micro-lamports <PRIORITY_FEE_MICRO_LAMPORTS>` — Priority fee in micro lamports

  Default value: `1`

- `--transaction-retries <TRANSACTION_RETRIES>` — Amount of times to retry a transaction

  Default value: `0`

- `--ncn-program-id <NCN_PROGRAM_ID>` — NCN program ID

  Default value: `7rNw1g2ZUCdTrCyVGZwCJLnbp3ssTRK5mdkH8gm9AKE8`

- `--restaking-program-id <RESTAKING_PROGRAM_ID>` — Restaking program ID

  Default value: `RestkWeAVL8fRGgzhfeoqFhsqKRchg6aa1XrcH96z4Q`

- `--vault-program-id <VAULT_PROGRAM_ID>` — Vault program ID

  Default value: `Vau1t6sLNxnzB7ZDsef8TLbPLfyZMYXH8WTNqUdm9g8`

- `--token-program-id <TOKEN_PROGRAM_ID>` — Token Program ID

  Default value: `TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA`

- `--ncn <NCN>` — NCN Account Address
- `--operator <OPERATOR>` — Operator Account Address
- `--epoch <EPOCH>` — Epoch - defaults to current epoch
- `--keypair-path <KEYPAIR_PATH>` — keypair path
- `--verbose` — Verbose mode

## `ncn-operator-cli keeper`

Keeper

**Usage:** `ncn-operator-cli keeper [OPTIONS] --cluster <CLUSTER>`

###### **Options:**

- `--loop-timeout-ms <LOOP_TIMEOUT_MS>` — Keeper error timeout in milliseconds

  Default value: `600000`

- `--error-timeout-ms <ERROR_TIMEOUT_MS>` — Keeper error timeout in milliseconds

  Default value: `10000`

- `--test-vote` — Calls test vote, instead of waiting for a real vote
- `--metrics-only` — Only emit metrics
- `--cluster <CLUSTER>` — Cluster label for metrics purposes

  Possible values: `mainnet`, `testnet`, `localnet`

- `--region <REGION>` — Region for metrics purposes

  Default value: `local`

## `ncn-operator-cli operator-cast-vote`

Instructions

**Usage:** `ncn-operator-cli operator-cast-vote --weather-status <WEATHER_STATUS>`

###### **Options:**

- `--weather-status <WEATHER_STATUS>` — Meta merkle root

## `ncn-operator-cli get-ncn`

Getters

**Usage:** `ncn-operator-cli get-ncn`

## `ncn-operator-cli get-ncn-operator-state`

**Usage:** `ncn-operator-cli get-ncn-operator-state`

## `ncn-operator-cli get-vault-ncn-ticket`

**Usage:** `ncn-operator-cli get-vault-ncn-ticket --vault <VAULT>`

###### **Options:**

- `--vault <VAULT>` — Vault Account Address

## `ncn-operator-cli get-ncn-vault-ticket`

**Usage:** `ncn-operator-cli get-ncn-vault-ticket --vault <VAULT>`

###### **Options:**

- `--vault <VAULT>` — Vault Account Address

## `ncn-operator-cli get-vault-operator-delegation`

**Usage:** `ncn-operator-cli get-vault-operator-delegation --vault <VAULT>`

###### **Options:**

- `--vault <VAULT>` — Vault Account Address

## `ncn-operator-cli get-all-tickets`

**Usage:** `ncn-operator-cli get-all-tickets`

## `ncn-operator-cli get-all-operators-in-ncn`

**Usage:** `ncn-operator-cli get-all-operators-in-ncn`

## `ncn-operator-cli get-all-vaults-in-ncn`

**Usage:** `ncn-operator-cli get-all-vaults-in-ncn`

## `ncn-operator-cli get-ncn-program-config`

**Usage:** `ncn-operator-cli get-ncn-program-config`

## `ncn-operator-cli get-vault-registry`

**Usage:** `ncn-operator-cli get-vault-registry`

## `ncn-operator-cli get-weight-table`

**Usage:** `ncn-operator-cli get-weight-table`

## `ncn-operator-cli get-epoch-state`

**Usage:** `ncn-operator-cli get-epoch-state`

## `ncn-operator-cli get-epoch-snapshot`

**Usage:** `ncn-operator-cli get-epoch-snapshot`

## `ncn-operator-cli get-operator-snapshot`

**Usage:** `ncn-operator-cli get-operator-snapshot`

## `ncn-operator-cli get-ballot-box`

**Usage:** `ncn-operator-cli get-ballot-box`

## `ncn-operator-cli get-account-payer`

**Usage:** `ncn-operator-cli get-account-payer`

## `ncn-operator-cli get-total-epoch-rent-cost`

**Usage:** `ncn-operator-cli get-total-epoch-rent-cost`

## `ncn-operator-cli get-consensus-result`

**Usage:** `ncn-operator-cli get-consensus-result`

## `ncn-operator-cli get-operator-stakes`

**Usage:** `ncn-operator-cli get-operator-stakes`

## `ncn-operator-cli get-vault-stakes`

**Usage:** `ncn-operator-cli get-vault-stakes`

## `ncn-operator-cli get-vault-operator-stakes`

**Usage:** `ncn-operator-cli get-vault-operator-stakes`

<hr/>

<small><i>
This document was generated automatically by
<a href="https://crates.io/crates/clap-markdown"><code>clap-markdown</code></a>.
</i></small>
