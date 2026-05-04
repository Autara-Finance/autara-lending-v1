/**
 * PDA derivation — direct port of autara-lib/src/pda.rs.
 *
 * Seed layout must match the Rust program byte-for-byte:
 *   market:          ["market", curator, supply_mint, collateral_mint, index]
 *   supply_position: ["supply_position", market, authority]
 *   borrow_position: ["borrow_position", market, authority]
 *   global_config:   ["global_config"]
 */

import type { PublicKeyStr } from "../../domain/types/common";

// These constants match the Rust seed literals exactly
const MARKET_SEED = new TextEncoder().encode("market");
const SUPPLY_POSITION_SEED = new TextEncoder().encode("supply_position");
const BORROW_POSITION_SEED = new TextEncoder().encode("borrow_position");
const GLOBAL_CONFIG_SEED = new TextEncoder().encode("global_config");

/**
 * Generic PDA finder — wraps the platform-specific findProgramAddress.
 * Accepts a function that does the actual cryptographic derivation
 * so this module stays dependency-free.
 */
export type FindProgramAddressFn = (
  seeds: Uint8Array[],
  programId: Uint8Array,
) => [Uint8Array, number];

export interface PdaDeriver {
  findMarketPda(
    curator: PublicKeyStr,
    supplyMint: PublicKeyStr,
    collateralMint: PublicKeyStr,
    index: number,
  ): [PublicKeyStr, number];

  findSupplyPositionPda(
    market: PublicKeyStr,
    authority: PublicKeyStr,
  ): [PublicKeyStr, number];

  findBorrowPositionPda(
    market: PublicKeyStr,
    authority: PublicKeyStr,
  ): [PublicKeyStr, number];

  findGlobalConfigPda(): [PublicKeyStr, number];
}

/**
 * Create a PDA deriver for a given program ID.
 *
 * @param programId - The Autara program's public key as bytes
 * @param findProgramAddress - Platform-specific PDA derivation (e.g. from @solana/web3.js or arch-sdk)
 * @param encodeKey - Convert PublicKeyStr to bytes
 * @param decodeKey - Convert bytes back to PublicKeyStr
 */
export function createPdaDeriver(
  programId: Uint8Array,
  findProgramAddress: FindProgramAddressFn,
  encodeKey: (key: PublicKeyStr) => Uint8Array,
  decodeKey: (bytes: Uint8Array) => PublicKeyStr,
): PdaDeriver {
  return {
    findMarketPda(curator, supplyMint, collateralMint, index) {
      const [address, bump] = findProgramAddress(
        [
          MARKET_SEED,
          encodeKey(curator),
          encodeKey(supplyMint),
          encodeKey(collateralMint),
          new Uint8Array([index]),
        ],
        programId,
      );
      return [decodeKey(address), bump];
    },

    findSupplyPositionPda(market, authority) {
      const [address, bump] = findProgramAddress(
        [SUPPLY_POSITION_SEED, encodeKey(market), encodeKey(authority)],
        programId,
      );
      return [decodeKey(address), bump];
    },

    findBorrowPositionPda(market, authority) {
      const [address, bump] = findProgramAddress(
        [BORROW_POSITION_SEED, encodeKey(market), encodeKey(authority)],
        programId,
      );
      return [decodeKey(address), bump];
    },

    findGlobalConfigPda() {
      const [address, bump] = findProgramAddress(
        [GLOBAL_CONFIG_SEED],
        programId,
      );
      return [decodeKey(address), bump];
    },
  };
}
