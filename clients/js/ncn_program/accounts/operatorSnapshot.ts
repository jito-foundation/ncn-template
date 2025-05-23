/**
 * This code was AUTOGENERATED using the kinobi library.
 * Please DO NOT EDIT THIS FILE, instead use visitors
 * to add features, then rerun kinobi to update it.
 *
 * @see https://github.com/kinobi-so/kinobi
 */

import {
  assertAccountExists,
  assertAccountsExist,
  combineCodec,
  decodeAccount,
  fetchEncodedAccount,
  fetchEncodedAccounts,
  getAddressDecoder,
  getAddressEncoder,
  getArrayDecoder,
  getArrayEncoder,
  getBoolDecoder,
  getBoolEncoder,
  getStructDecoder,
  getStructEncoder,
  getU16Decoder,
  getU16Encoder,
  getU64Decoder,
  getU64Encoder,
  getU8Decoder,
  getU8Encoder,
  type Account,
  type Address,
  type Codec,
  type Decoder,
  type EncodedAccount,
  type Encoder,
  type FetchAccountConfig,
  type FetchAccountsConfig,
  type MaybeAccount,
  type MaybeEncodedAccount,
} from '@solana/web3.js';
import {
  getStakeWeightsDecoder,
  getStakeWeightsEncoder,
  getVaultOperatorStakeWeightDecoder,
  getVaultOperatorStakeWeightEncoder,
  type StakeWeights,
  type StakeWeightsArgs,
  type VaultOperatorStakeWeight,
  type VaultOperatorStakeWeightArgs,
} from '../types';

export type OperatorSnapshot = {
  discriminator: bigint;
  operator: Address;
  ncn: Address;
  ncnEpoch: bigint;
  bump: number;
  slotCreated: bigint;
  slotFinalized: bigint;
  isActive: number;
  ncnOperatorIndex: bigint;
  operatorIndex: bigint;
  operatorFeeBps: number;
  vaultOperatorDelegationCount: bigint;
  vaultOperatorDelegationsRegistered: bigint;
  validOperatorVaultDelegations: bigint;
  stakeWeights: StakeWeights;
  vaultOperatorStakeWeight: Array<VaultOperatorStakeWeight>;
};

export type OperatorSnapshotArgs = {
  discriminator: number | bigint;
  operator: Address;
  ncn: Address;
  ncnEpoch: number | bigint;
  bump: number;
  slotCreated: number | bigint;
  slotFinalized: number | bigint;
  isActive: number;
  ncnOperatorIndex: number | bigint;
  operatorIndex: number | bigint;
  operatorFeeBps: number;
  vaultOperatorDelegationCount: number | bigint;
  vaultOperatorDelegationsRegistered: number | bigint;
  validOperatorVaultDelegations: number | bigint;
  stakeWeights: StakeWeightsArgs;
  vaultOperatorStakeWeight: Array<VaultOperatorStakeWeightArgs>;
};

export function getOperatorSnapshotEncoder(): Encoder<OperatorSnapshotArgs> {
  return getStructEncoder([
    ['discriminator', getU64Encoder()],
    ['operator', getAddressEncoder()],
    ['ncn', getAddressEncoder()],
    ['ncnEpoch', getU64Encoder()],
    ['bump', getU8Encoder()],
    ['slotCreated', getU64Encoder()],
    ['slotFinalized', getU64Encoder()],
    ['isActive', getBoolEncoder()],
    ['ncnOperatorIndex', getU64Encoder()],
    ['operatorIndex', getU64Encoder()],
    ['operatorFeeBps', getU16Encoder()],
    ['vaultOperatorDelegationCount', getU64Encoder()],
    ['vaultOperatorDelegationsRegistered', getU64Encoder()],
    ['validOperatorVaultDelegations', getU64Encoder()],
    ['stakeWeights', getStakeWeightsEncoder()],
    [
      'vaultOperatorStakeWeight',
      getArrayEncoder(getVaultOperatorStakeWeightEncoder(), { size: 64 }),
    ],
  ]);
}

export function getOperatorSnapshotDecoder(): Decoder<OperatorSnapshot> {
  return getStructDecoder([
    ['discriminator', getU64Decoder()],
    ['operator', getAddressDecoder()],
    ['ncn', getAddressDecoder()],
    ['ncnEpoch', getU64Decoder()],
    ['bump', getU8Decoder()],
    ['slotCreated', getU64Decoder()],
    ['slotFinalized', getU64Decoder()],
    ['isActive', getBoolDecoder()],
    ['ncnOperatorIndex', getU64Decoder()],
    ['operatorIndex', getU64Decoder()],
    ['operatorFeeBps', getU16Decoder()],
    ['vaultOperatorDelegationCount', getU64Decoder()],
    ['vaultOperatorDelegationsRegistered', getU64Decoder()],
    ['validOperatorVaultDelegations', getU64Decoder()],
    ['stakeWeights', getStakeWeightsDecoder()],
    [
      'vaultOperatorStakeWeight',
      getArrayDecoder(getVaultOperatorStakeWeightDecoder(), { size: 64 }),
    ],
  ]);
}

export function getOperatorSnapshotCodec(): Codec<
  OperatorSnapshotArgs,
  OperatorSnapshot
> {
  return combineCodec(
    getOperatorSnapshotEncoder(),
    getOperatorSnapshotDecoder()
  );
}

export function decodeOperatorSnapshot<TAddress extends string = string>(
  encodedAccount: EncodedAccount<TAddress>
): Account<OperatorSnapshot, TAddress>;
export function decodeOperatorSnapshot<TAddress extends string = string>(
  encodedAccount: MaybeEncodedAccount<TAddress>
): MaybeAccount<OperatorSnapshot, TAddress>;
export function decodeOperatorSnapshot<TAddress extends string = string>(
  encodedAccount: EncodedAccount<TAddress> | MaybeEncodedAccount<TAddress>
):
  | Account<OperatorSnapshot, TAddress>
  | MaybeAccount<OperatorSnapshot, TAddress> {
  return decodeAccount(
    encodedAccount as MaybeEncodedAccount<TAddress>,
    getOperatorSnapshotDecoder()
  );
}

export async function fetchOperatorSnapshot<TAddress extends string = string>(
  rpc: Parameters<typeof fetchEncodedAccount>[0],
  address: Address<TAddress>,
  config?: FetchAccountConfig
): Promise<Account<OperatorSnapshot, TAddress>> {
  const maybeAccount = await fetchMaybeOperatorSnapshot(rpc, address, config);
  assertAccountExists(maybeAccount);
  return maybeAccount;
}

export async function fetchMaybeOperatorSnapshot<
  TAddress extends string = string,
>(
  rpc: Parameters<typeof fetchEncodedAccount>[0],
  address: Address<TAddress>,
  config?: FetchAccountConfig
): Promise<MaybeAccount<OperatorSnapshot, TAddress>> {
  const maybeAccount = await fetchEncodedAccount(rpc, address, config);
  return decodeOperatorSnapshot(maybeAccount);
}

export async function fetchAllOperatorSnapshot(
  rpc: Parameters<typeof fetchEncodedAccounts>[0],
  addresses: Array<Address>,
  config?: FetchAccountsConfig
): Promise<Account<OperatorSnapshot>[]> {
  const maybeAccounts = await fetchAllMaybeOperatorSnapshot(
    rpc,
    addresses,
    config
  );
  assertAccountsExist(maybeAccounts);
  return maybeAccounts;
}

export async function fetchAllMaybeOperatorSnapshot(
  rpc: Parameters<typeof fetchEncodedAccounts>[0],
  addresses: Array<Address>,
  config?: FetchAccountsConfig
): Promise<MaybeAccount<OperatorSnapshot>[]> {
  const maybeAccounts = await fetchEncodedAccounts(rpc, addresses, config);
  return maybeAccounts.map((maybeAccount) =>
    decodeOperatorSnapshot(maybeAccount)
  );
}
