#!/usr/bin/env bash

SBF_PROGRAM_DIR=$PWD/integration_tests/tests/fixtures
FIXTURES_DIR=$PWD/integration_tests/tests/fixtures
LEDGER_DIR=$FIXTURES_DIR/test-ledger
DESIRED_SLOT=150
max_validators=10
validator_file=$FIXTURES_DIR/local_validators.txt
sol_amount=50
stake_per_validator=$((($sol_amount - ($max_validators * 2)) / $max_validators))

keys_dir=$FIXTURES_DIR/keys
mkdir -p $keys_dir

create_keypair() {
	if test ! -f "$1"; then
		solana-keygen new --no-passphrase -s -o "$1"
	fi
}

# Function to create keypairs and serialize accounts
prepare_keypairs() {
	max_validators=$1
	validator_file=$2

	for number in $(seq 1 "$max_validators"); do
		# Create keypairs for identity, vote, and withdrawer
		create_keypair "$keys_dir/identity_$number.json"
		create_keypair "$keys_dir/vote_$number.json"
		create_keypair "$keys_dir/withdrawer_$number.json"

		# Get the public key of the vote account
		vote_pubkey=$(solana-keygen pubkey "$keys_dir/vote_$number.json")

		# Extract the public key from the identity keypair
		merkle_root_upload_authority=$(solana-keygen pubkey "$keys_dir/identity_$number.json")

		# Append the vote public key to the validator file
		echo "$vote_pubkey" >>"$validator_file"
	done
}

# Function to create vote accounts
create_vote_accounts() {
	max_validators=$1
	validator_file=$2

	for number in $(seq 1 "$max_validators"); do
		# Create the vote account
		solana create-vote-account \
			"$keys_dir/vote_$number.json" \
			"$keys_dir/identity_$number.json" \
			"$keys_dir/withdrawer_$number.json" \
			--commission 1
	done
}

# Hoist the creation of keypairs and serialization
echo "Preparing keypairs and serializing accounts"
prepare_keypairs "$max_validators" "$validator_file"

echo "tda_account_args ${tda_account_args[@]}"

VALIDATOR_PID=
setup_test_validator() {
	solana-test-validator \
		--account-dir $FIXTURES_DIR/accounts \
		"${tda_account_args[@]}" \
		--ledger $LEDGER_DIR \
		--slots-per-epoch 32 \
		--quiet --reset &
	VALIDATOR_PID=$!
	solana config set --url http://127.0.0.1:8899
	solana config set --commitment confirmed
	echo "waiting for solana-test-validator, pid: $VALIDATOR_PID"
	sleep 15
}

echo "Setting up local test validator"
set +ex
setup_test_validator
set -ex

echo "Creating vote accounts"
create_vote_accounts "$max_validators" "$validator_file"

echo "Done adding $max_validators validator vote accounts, their pubkeys can be found in $validator_file"

command_args=()

###################################################
### MODIFY PARAMETERS BELOW THIS LINE FOR YOUR POOL
###################################################

# Epoch fee, assessed as a percentage of rewards earned by the pool every epoch,
# represented as `numerator / denominator`
command_args+=(--epoch-fee-numerator 1)
command_args+=(--epoch-fee-denominator 100)

# Withdrawal fee for SOL and stake accounts, represented as `numerator / denominator`
command_args+=(--withdrawal-fee-numerator 2)
command_args+=(--withdrawal-fee-denominator 100)

# Deposit fee for SOL and stake accounts, represented as `numerator / denominator`
command_args+=(--deposit-fee-numerator 3)
command_args+=(--deposit-fee-denominator 100)

command_args+=(--referral-fee 0) # Percentage of deposit fee that goes towards the referrer (a number between 0 and 100, inclusive)

command_args+=(--max-validators 2350) # Maximum number of validators in the stake pool, 2350 is the current maximum possible

# (Optional) Deposit authority, required to sign all deposits into the pool.
# Setting this variable makes the pool "private" or "restricted".
# Uncomment and set to a valid keypair if you want the pool to be restricted.
#command_args+=( --deposit-authority keys/authority.json )

###################################################
### MODIFY PARAMETERS ABOVE THIS LINE FOR YOUR POOL
###################################################

echo "Creating pool"
validator_list_keyfile=$keys_dir/validator-list.json
mint_keyfile=$keys_dir/mint.json
reserve_keyfile=$keys_dir/reserve.json
create_keypair $validator_list_keyfile
create_keypair $mint_keyfile
create_keypair $reserve_keyfile

set +ex
lst_mint_pubkey=$(solana-keygen pubkey "$mint_keyfile")
set -ex

# Clear the validator vote pubkey file so it doesn't expand and cause errors next run
rm $validator_file

# wait for certain epoch
echo "waiting for epoch X from validator $VALIDATOR_PID"
while true; do
	current_slot=$(solana slot --url http://localhost:8899)
	echo "current slot $current_slot"
	[[ $current_slot -gt $DESIRED_SLOT ]] && kill $VALIDATOR_PID && exit 0
	sleep 5
done
