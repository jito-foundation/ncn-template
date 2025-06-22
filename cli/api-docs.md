---
title: CLI
category: Jekyll
layout: post
weight: 1
---

# Command-Line Help for `ncn-program-cli`

This document contains the help content for the `ncn-program-cli` command-line program.

## `ncn-program-cli`

A CLI for creating and managing the ncn program

**Usage:** `ncn-program-cli [OPTIONS] <COMMAND>`

###### **Subcommands:**

* `run-keeper` — NCN Keeper
* `run-operator` — Operator Keeper
* `crank-update-all-vaults` — Crank Functions
* `crank-register-vaults` — 
* `crank-snapshot` — 
* `crank-distribute` — 
* `crank-close-epoch-accounts` — 
* `set-epoch-weights` — 
* `admin-create-config` — Admin
* `admin-register-st-mint` — 
* `admin-set-weight` — 
* `admin-set-tie-breaker` — 
* `admin-set-parameters` — 
* `admin-set-new-admin` — 
* `admin-fund-account-payer` — 
* `create-vault-registry` — Instructions
* `register-vault` — 
* `create-epoch-state` — 
* `create-weight-table` — 
* `create-epoch-snapshot` — 
* `create-operator-snapshot` — 
* `snapshot-vault-operator-delegation` — 
* `create-ballot-box` — 
* `operator-cast-vote` — 
* `create-ncn-reward-router` — 
* `create-operator-vault-reward-router` — 
* `route-ncn-rewards` — 
* `route-operator-vault-rewards` — 
* `distribute-base-operator-vault-rewards` — 
* `get-ncn` — Getters
* `get-ncn-operator-state` — 
* `get-vault-ncn-ticket` — 
* `get-ncn-vault-ticket` — 
* `get-vault-operator-delegation` — 
* `get-all-tickets` — 
* `get-all-operators-in-ncn` — 
* `get-all-vaults-in-ncn` — 
* `get-ncn-program-config` — 
* `get-vault-registry` — 
* `get-weight-table` — 
* `get-epoch-state` — 
* `get-epoch-snapshot` — 
* `get-operator-snapshot` — 
* `get-ballot-box` — 
* `get-account-payer` — 
* `get-total-epoch-rent-cost` — 
* `get-consensus-result` — 
* `get-operator-stakes` — 
* `get-vault-stakes` — 
* `get-vault-operator-stakes` — 
* `get-ncn-reward-router` — 
* `get-ncn-reward-receiver-address` — 
* `get-operator-vault-reward-router` — 
* `get-all-operator-vault-reward-routers` — 
* `full-update-vaults` — 

###### **Options:**

* `--rpc-url <RPC_URL>` — RPC URL to use

  Default value: `https://api.mainnet-beta.solana.com`
* `--commitment <COMMITMENT>` — Commitment level

  Default value: `confirmed`
* `--priority-fee-micro-lamports <PRIORITY_FEE_MICRO_LAMPORTS>` — Priority fee in micro lamports

  Default value: `1`
* `--transaction-retries <TRANSACTION_RETRIES>` — Amount of times to retry a transaction

  Default value: `0`
* `--ncn-program-id <NCN_PROGRAM_ID>` — NCN program ID

  Default value: `5SiK283D1iFSqHvr8vbNWCBjbjRXeEYS79CLax7nosPf`
* `--restaking-program-id <RESTAKING_PROGRAM_ID>` — Restaking program ID

  Default value: `RestkWeAVL8fRGgzhfeoqFhsqKRchg6aa1XrcH96z4Q`
* `--vault-program-id <VAULT_PROGRAM_ID>` — Vault program ID

  Default value: `Vau1t6sLNxnzB7ZDsef8TLbPLfyZMYXH8WTNqUdm9g8`
* `--token-program-id <TOKEN_PROGRAM_ID>` — Token Program ID

  Default value: `TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA`
* `--ncn <NCN>` — NCN Account Address
* `--epoch <EPOCH>` — Epoch - defaults to current epoch
* `--keypair-path <KEYPAIR_PATH>` — keypair path
* `--verbose` — Verbose mode
* `--open-weather-api-key <OPEN_WEATHER_API_KEY>` — Open weather api key



## `ncn-program-cli run-keeper`

NCN Keeper - Automates the epoch lifecycle including set weights, snapshots, voting, distribution, and closing

**Usage:** `ncn-program-cli run-keeper [OPTIONS]`

###### **Options:**

* `--loop-timeout-ms <LOOP_TIMEOUT_MS>` — Keeper error timeout in milliseconds

  Default value: `600000`
* `--error-timeout-ms <ERROR_TIMEOUT_MS>` — Keeper error timeout in milliseconds

  Default value: `10000`



## `ncn-program-cli run-operator`

Operator Keeper

**Usage:** `ncn-program-cli run-operator [OPTIONS] --operator <OPERATOR>`

###### **Options:**

* `--operator <OPERATOR>` — Operator address
* `--loop-timeout-ms <LOOP_TIMEOUT_MS>` — Keeper error timeout in milliseconds

  Default value: `600000`
* `--error-timeout-ms <ERROR_TIMEOUT_MS>` — Keeper error timeout in milliseconds

  Default value: `10000`



## `ncn-program-cli crank-update-all-vaults`

Crank Functions

**Usage:** `ncn-program-cli crank-update-all-vaults`



## `ncn-program-cli crank-register-vaults`

**Usage:** `ncn-program-cli crank-register-vaults`



## `ncn-program-cli crank-snapshot`

**Usage:** `ncn-program-cli crank-snapshot`



## `ncn-program-cli crank-distribute`

Handles the distribution of rewards after consensus is reached

**Usage:** `ncn-program-cli crank-distribute`



## `ncn-program-cli crank-close-epoch-accounts`

**Usage:** `ncn-program-cli crank-close-epoch-accounts`



## `ncn-program-cli set-epoch-weights`

**Usage:** `ncn-program-cli set-epoch-weights`



## `ncn-program-cli admin-create-config`

Admin

**Usage:** `ncn-program-cli admin-create-config [OPTIONS] --ncn-fee-wallet <NCN_FEE_WALLET> --ncn-fee-bps <NCN_FEE_BPS>`

###### **Options:**

* `--ncn-fee-wallet <NCN_FEE_WALLET>` — Ncn Fee Wallet Address
* `--ncn-fee-bps <NCN_FEE_BPS>` — Ncn Fee bps
* `--epochs-before-stall <EPOCHS_BEFORE_STALL>` — Epochs before tie breaker can set consensus

  Default value: `10`
* `--valid-slots-after-consensus <VALID_SLOTS_AFTER_CONSENSUS>` — Valid slots after consensus

  Default value: `43200`
* `--epochs-after-consensus-before-close <EPOCHS_AFTER_CONSENSUS_BEFORE_CLOSE>` — Epochs after consensus before accounts can be closed

  Default value: `10`
* `--tie-breaker-admin <TIE_BREAKER_ADMIN>` — Tie breaker admin address



## `ncn-program-cli admin-register-st-mint`

**Usage:** `ncn-program-cli admin-register-st-mint [OPTIONS] --vault <VAULT>`

###### **Options:**

* `--vault <VAULT>` — Vault address
* `--weight <WEIGHT>` — Weight



## `ncn-program-cli admin-set-weight`

**Usage:** `ncn-program-cli admin-set-weight --vault <VAULT> --weight <WEIGHT>`

###### **Options:**

* `--vault <VAULT>` — Vault address
* `--weight <WEIGHT>` — Weight value



## `ncn-program-cli admin-set-tie-breaker`

**Usage:** `ncn-program-cli admin-set-tie-breaker --weather-status <WEATHER_STATUS>`

###### **Options:**

* `--weather-status <WEATHER_STATUS>` — tir breaker for voting



## `ncn-program-cli admin-set-parameters`

**Usage:** `ncn-program-cli admin-set-parameters [OPTIONS]`

###### **Options:**

* `--epochs-before-stall <EPOCHS_BEFORE_STALL>` — Epochs before tie breaker can set consensus
* `--epochs-after-consensus-before-close <EPOCHS_AFTER_CONSENSUS_BEFORE_CLOSE>` — Epochs after consensus before accounts can be closed
* `--valid-slots-after-consensus <VALID_SLOTS_AFTER_CONSENSUS>` — Slots to which voting is allowed after consensus
* `--starting-valid-epoch <STARTING_VALID_EPOCH>` — Starting valid epoch



## `ncn-program-cli admin-set-new-admin`

**Usage:** `ncn-program-cli admin-set-new-admin [OPTIONS] --new-admin <NEW_ADMIN>`

###### **Options:**

* `--new-admin <NEW_ADMIN>` — New admin address
* `--set-tie-breaker-admin` — Set tie breaker admin



## `ncn-program-cli admin-fund-account-payer`

**Usage:** `ncn-program-cli admin-fund-account-payer --amount-in-sol <AMOUNT_IN_SOL>`

###### **Options:**

* `--amount-in-sol <AMOUNT_IN_SOL>` — Amount of SOL to fund



## `ncn-program-cli create-vault-registry`

Instructions

**Usage:** `ncn-program-cli create-vault-registry`



## `ncn-program-cli register-vault`

**Usage:** `ncn-program-cli register-vault --vault <VAULT>`

###### **Options:**

* `--vault <VAULT>` — Vault address



## `ncn-program-cli create-epoch-state`

**Usage:** `ncn-program-cli create-epoch-state`



## `ncn-program-cli create-weight-table`

**Usage:** `ncn-program-cli create-weight-table`



## `ncn-program-cli create-epoch-snapshot`

**Usage:** `ncn-program-cli create-epoch-snapshot`



## `ncn-program-cli create-operator-snapshot`

**Usage:** `ncn-program-cli create-operator-snapshot --operator <OPERATOR>`

###### **Options:**

* `--operator <OPERATOR>` — Operator address



## `ncn-program-cli snapshot-vault-operator-delegation`

**Usage:** `ncn-program-cli snapshot-vault-operator-delegation --vault <VAULT> --operator <OPERATOR>`

###### **Options:**

* `--vault <VAULT>` — Vault address
* `--operator <OPERATOR>` — Operator address



## `ncn-program-cli create-ballot-box`

**Usage:** `ncn-program-cli create-ballot-box`



## `ncn-program-cli operator-cast-vote`

**Usage:** `ncn-program-cli operator-cast-vote --operator <OPERATOR> --weather-status <WEATHER_STATUS>`

###### **Options:**

* `--operator <OPERATOR>` — Operator address
* `--weather-status <WEATHER_STATUS>` — weather status at solana beach



## `ncn-program-cli create-ncn-reward-router`

**Usage:** `ncn-program-cli create-ncn-reward-router`



## `ncn-program-cli create-operator-vault-reward-router`

**Usage:** `ncn-program-cli create-operator-vault-reward-router --operator <OPERATOR>`

###### **Options:**

* `--operator <OPERATOR>` — Operator address



## `ncn-program-cli route-ncn-rewards`

**Usage:** `ncn-program-cli route-ncn-rewards`



## `ncn-program-cli route-operator-vault-rewards`

**Usage:** `ncn-program-cli route-operator-vault-rewards --operator <OPERATOR>`

###### **Options:**

* `--operator <OPERATOR>` — Operator address



## `ncn-program-cli distribute-base-operator-vault-rewards`

**Usage:** `ncn-program-cli distribute-base-operator-vault-rewards --operator <OPERATOR>`

###### **Options:**

* `--operator <OPERATOR>` — Operator address



## `ncn-program-cli get-ncn`

Getters

**Usage:** `ncn-program-cli get-ncn`



## `ncn-program-cli get-ncn-operator-state`

**Usage:** `ncn-program-cli get-ncn-operator-state --operator <OPERATOR>`

###### **Options:**

* `--operator <OPERATOR>` — Operator Account Address



## `ncn-program-cli get-vault-ncn-ticket`

**Usage:** `ncn-program-cli get-vault-ncn-ticket --vault <VAULT>`

###### **Options:**

* `--vault <VAULT>` — Vault Account Address



## `ncn-program-cli get-ncn-vault-ticket`

**Usage:** `ncn-program-cli get-ncn-vault-ticket --vault <VAULT>`

###### **Options:**

* `--vault <VAULT>` — Vault Account Address



## `ncn-program-cli get-vault-operator-delegation`

**Usage:** `ncn-program-cli get-vault-operator-delegation --vault <VAULT> --operator <OPERATOR>`

###### **Options:**

* `--vault <VAULT>` — Vault Account Address
* `--operator <OPERATOR>` — Operator Account Address



## `ncn-program-cli get-all-tickets`

**Usage:** `ncn-program-cli get-all-tickets`



## `ncn-program-cli get-all-operators-in-ncn`

**Usage:** `ncn-program-cli get-all-operators-in-ncn`



## `ncn-program-cli get-all-vaults-in-ncn`

**Usage:** `ncn-program-cli get-all-vaults-in-ncn`



## `ncn-program-cli get-ncn-program-config`

**Usage:** `ncn-program-cli get-ncn-program-config`



## `ncn-program-cli get-vault-registry`

**Usage:** `ncn-program-cli get-vault-registry`



## `ncn-program-cli get-weight-table`

**Usage:** `ncn-program-cli get-weight-table`



## `ncn-program-cli get-epoch-state`

**Usage:** `ncn-program-cli get-epoch-state`



## `ncn-program-cli get-epoch-snapshot`

**Usage:** `ncn-program-cli get-epoch-snapshot`



## `ncn-program-cli get-operator-snapshot`

**Usage:** `ncn-program-cli get-operator-snapshot --operator <OPERATOR>`

###### **Options:**

* `--operator <OPERATOR>` — Operator Account Address



## `ncn-program-cli get-ballot-box`

**Usage:** `ncn-program-cli get-ballot-box`



## `ncn-program-cli get-account-payer`

**Usage:** `ncn-program-cli get-account-payer`



## `ncn-program-cli get-total-epoch-rent-cost`

**Usage:** `ncn-program-cli get-total-epoch-rent-cost`



## `ncn-program-cli get-consensus-result`

**Usage:** `ncn-program-cli get-consensus-result`



## `ncn-program-cli get-operator-stakes`

**Usage:** `ncn-program-cli get-operator-stakes`



## `ncn-program-cli get-vault-stakes`

**Usage:** `ncn-program-cli get-vault-stakes`



## `ncn-program-cli get-vault-operator-stakes`

**Usage:** `ncn-program-cli get-vault-operator-stakes`



## `ncn-program-cli get-ncn-reward-router`

**Usage:** `ncn-program-cli get-ncn-reward-router`



## `ncn-program-cli get-ncn-reward-receiver-address`

**Usage:** `ncn-program-cli get-ncn-reward-receiver-address`



## `ncn-program-cli get-operator-vault-reward-router`

**Usage:** `ncn-program-cli get-operator-vault-reward-router --operator <OPERATOR>`

###### **Options:**

* `--operator <OPERATOR>` — Operator Account Address



## `ncn-program-cli get-all-operator-vault-reward-routers`

**Usage:** `ncn-program-cli get-all-operator-vault-reward-routers`



## `ncn-program-cli full-update-vaults`

**Usage:** `ncn-program-cli full-update-vaults [OPTIONS]`

###### **Options:**

* `--vault <VAULT>` — Vault address



<hr/>

<small><i>
    This document was generated automatically by
    <a href="https://crates.io/crates/clap-markdown"><code>clap-markdown</code></a>.
</i></small>

