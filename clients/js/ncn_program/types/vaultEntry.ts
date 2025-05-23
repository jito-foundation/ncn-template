/**
 * This code was AUTOGENERATED using the kinobi library.
 * Please DO NOT EDIT THIS FILE, instead use visitors
 * to add features, then rerun kinobi to update it.
 *
 * @see https://github.com/kinobi-so/kinobi
 */

import {
  combineCodec,
  getAddressDecoder,
  getAddressEncoder,
  getStructDecoder,
  getStructEncoder,
  getU64Decoder,
  getU64Encoder,
  type Address,
  type Codec,
  type Decoder,
  type Encoder,
} from '@solana/web3.js';

export type VaultEntry = {
  vault: Address;
  stMint: Address;
  vaultIndex: bigint;
  slotRegistered: bigint;
};

export type VaultEntryArgs = {
  vault: Address;
  stMint: Address;
  vaultIndex: number | bigint;
  slotRegistered: number | bigint;
};

export function getVaultEntryEncoder(): Encoder<VaultEntryArgs> {
  return getStructEncoder([
    ['vault', getAddressEncoder()],
    ['stMint', getAddressEncoder()],
    ['vaultIndex', getU64Encoder()],
    ['slotRegistered', getU64Encoder()],
  ]);
}

export function getVaultEntryDecoder(): Decoder<VaultEntry> {
  return getStructDecoder([
    ['vault', getAddressDecoder()],
    ['stMint', getAddressDecoder()],
    ['vaultIndex', getU64Decoder()],
    ['slotRegistered', getU64Decoder()],
  ]);
}

export function getVaultEntryCodec(): Codec<VaultEntryArgs, VaultEntry> {
  return combineCodec(getVaultEntryEncoder(), getVaultEntryDecoder());
}
