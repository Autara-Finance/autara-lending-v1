/**
 * Binary deserialization of on-chain account data into domain types.
 *
 * Layout sizes mirror the validate_struct! macros in Rust:
 *   Market          = 1448 bytes (MarketConfig=192 + CollateralVault=536 + SupplyVault=720)
 *   SupplyPosition  = 216 bytes
 *   BorrowPosition  = 224 bytes
 *
 * All structs are #[repr(C)] with bytemuck Pod — fields are laid out in declaration order
 * with no implicit padding (explicit Padding<N> fields handle alignment).
 *
 * NOTE: This is a structural skeleton. The actual byte-offset parsing must be validated
 * against the Rust struct layouts once the Arch SDK is integrated. The offsets below
 * are derived from the field sizes in the Rust source.
 */

import type { PublicKeyStr } from "../../domain/types/common";
import type { UFixedPoint, IFixedPoint } from "../../domain/types/fixed-point";
import type { Atoms } from "../../domain/types/atoms";
import { atoms } from "../../domain/types/atoms";
import { AutaraError, AutaraErrorCode } from "../../domain/errors";
import type { Market } from "../../domain/market/market";
import type { MarketConfig, LtvConfig } from "../../domain/market/market-config";
import type { SupplyVault } from "../../domain/market/supply-vault";
import type { CollateralVault } from "../../domain/market/collateral-vault";
import type { SupplyPosition } from "../../domain/lender/supply-position";
import type { BorrowPosition } from "../../domain/borrower/borrow-position";
import type { SharesTrackerView } from "../../domain/types/shares";

// ── Size constants ──────────────────────────────────────────────────────

export const MARKET_SIZE = 1448;
export const MARKET_CONFIG_SIZE = 192;
export const COLLATERAL_VAULT_SIZE = 536;
export const SUPPLY_VAULT_SIZE = 720;
export const SUPPLY_POSITION_SIZE = 216;
export const BORROW_POSITION_SIZE = 224;

// ── Primitive readers ───────────────────────────────────────────────────

type DecodeKeyFn = (bytes: Uint8Array) => PublicKeyStr;

function readU8(view: DataView, offset: number): number {
  return view.getUint8(offset);
}

function readU16LE(view: DataView, offset: number): number {
  return view.getUint16(offset, true);
}

function readU64LE(view: DataView, offset: number): bigint {
  return view.getBigUint64(offset, true);
}

function readI64LE(view: DataView, offset: number): bigint {
  return view.getBigInt64(offset, true);
}

/** Read a 128-bit unsigned integer (little-endian) as UFixedPoint */
function readU128LE(view: DataView, offset: number): UFixedPoint {
  const lo = view.getBigUint64(offset, true);
  const hi = view.getBigUint64(offset + 8, true);
  return ((hi << 64n) | lo) as UFixedPoint;
}

/** Read a 128-bit signed integer (little-endian) as IFixedPoint */
function readI128LE(view: DataView, offset: number): IFixedPoint {
  const lo = view.getBigUint64(offset, true);
  const hi = view.getBigInt64(offset + 8, true);
  return ((hi << 64n) | lo) as IFixedPoint;
}

function readPubkey(data: Uint8Array, offset: number, decodeKey: DecodeKeyFn): PublicKeyStr {
  return decodeKey(data.subarray(offset, offset + 32));
}

function readSharesTracker(view: DataView, offset: number): SharesTrackerView {
  return {
    totalShares: readU128LE(view, offset),         // 16 bytes
    atomsPerShare: readU128LE(view, offset + 16),  // 16 bytes
  };
}

// ── Public decoders ─────────────────────────────────────────────────────

function assertSize(label: string, data: Uint8Array, expected: number): void {
  if (data.length !== expected) {
    throw new AutaraError(
      AutaraErrorCode.DeserializationFailed,
      `Expected ${expected} bytes for ${label}, got ${data.length}`,
    );
  }
}

export function decodeSupplyPosition(
  address: PublicKeyStr,
  data: Uint8Array,
  decodeKey: DecodeKeyFn,
): SupplyPosition {
  assertSize("SupplyPosition", data, SUPPLY_POSITION_SIZE);
  const view = new DataView(data.buffer, data.byteOffset, data.byteLength);

  // Layout: authority(32) + market(32) + deposited_atoms(8) + shares(16) + pad(128)
  return {
    address,
    authority: readPubkey(data, 0, decodeKey),
    market: readPubkey(data, 32, decodeKey),
    depositedAtoms: atoms(readU64LE(view, 64)),
    shares: readU128LE(view, 72),
  };
}

export function decodeBorrowPosition(
  address: PublicKeyStr,
  data: Uint8Array,
  decodeKey: DecodeKeyFn,
): BorrowPosition {
  assertSize("BorrowPosition", data, BORROW_POSITION_SIZE);
  const view = new DataView(data.buffer, data.byteOffset, data.byteLength);

  // Layout: authority(32) + market(32) + collateral_deposited_atoms(8) + initial_borrowed_atoms(8) + borrowed_shares(16) + pad(128)
  return {
    address,
    authority: readPubkey(data, 0, decodeKey),
    market: readPubkey(data, 32, decodeKey),
    collateralDepositedAtoms: atoms(readU64LE(view, 64)),
    initialBorrowedAtoms: atoms(readU64LE(view, 72)),
    borrowedShares: readU128LE(view, 80),
  };
}

/**
 * Decode a full Market from raw account data.
 *
 * Layout: MarketConfig(192) + CollateralVault(536) + SupplyVault(720) = 1448 bytes
 *
 * NOTE: Oracle config and interest rate curve decoding are simplified here.
 * Full implementation requires matching the exact PodOracleProvider and
 * PodInterestRateCurve binary layouts from the Rust source.
 */
export function decodeMarket(
  address: PublicKeyStr,
  data: Uint8Array,
  decodeKey: DecodeKeyFn,
): Market {
  assertSize("Market", data, MARKET_SIZE);
  const view = new DataView(data.buffer, data.byteOffset, data.byteLength);

  const config = decodeMarketConfig(data.subarray(0, MARKET_CONFIG_SIZE), view, decodeKey);
  const collateralVault = decodeCollateralVault(
    data.subarray(MARKET_CONFIG_SIZE, MARKET_CONFIG_SIZE + COLLATERAL_VAULT_SIZE),
    new DataView(data.buffer, data.byteOffset + MARKET_CONFIG_SIZE, COLLATERAL_VAULT_SIZE),
    decodeKey,
  );
  const supplyVault = decodeSupplyVault(
    data.subarray(MARKET_CONFIG_SIZE + COLLATERAL_VAULT_SIZE),
    new DataView(
      data.buffer,
      data.byteOffset + MARKET_CONFIG_SIZE + COLLATERAL_VAULT_SIZE,
      SUPPLY_VAULT_SIZE,
    ),
    decodeKey,
  );

  return { address, config, collateralVault, supplyVault };
}

// ── Internal decoders ───────────────────────────────────────────────────

function decodeLtvConfig(view: DataView, offset: number): LtvConfig {
  return {
    maxLtv: readI128LE(view, offset),
    unhealthyLtv: readI128LE(view, offset + 16),
    liquidationBonus: readI128LE(view, offset + 32),
  };
}

function decodeMarketConfig(
  _data: Uint8Array,
  view: DataView,
  decodeKey: DecodeKeyFn,
): MarketConfig {
  // Layout: bump(1) + index(1) + lending_fee(2) + protocol_fee(2) + pad(2) + curator(32) + ltv_config(48) + max_util(16) + max_supply(8) + pad(80)
  return {
    bump: readU8(view, 0),
    index: readU8(view, 1),
    lendingMarketFeeInBps: readU16LE(view, 2),
    protocolFeeShareInBps: readU16LE(view, 4),
    curator: readPubkey(_data, 8, decodeKey),
    ltvConfig: decodeLtvConfig(view, 40),
    maxUtilisationRate: readI128LE(view, 88),
    maxSupplyAtoms: atoms(readU64LE(view, 104)),
  };
}

function decodeCollateralVault(
  data: Uint8Array,
  view: DataView,
  decodeKey: DecodeKeyFn,
): CollateralVault {
  // Layout: mint(32) + mint_decimals(8) + vault(32) + total_collateral_atoms(8) + oracle_config(264) + pad(192)
  return {
    mint: readPubkey(data, 0, decodeKey),
    mintDecimals: Number(readU64LE(view, 32)),
    vault: readPubkey(data, 40, decodeKey),
    totalCollateralAtoms: atoms(readU64LE(view, 72)),
    oracleConfig: {
      oracleProvider: { kind: "pyth", feedId: new Uint8Array(32), programId: readPubkey(data, 80, decodeKey) },
      validationConfig: { maxStalenessSeconds: 0, maxConfidenceToRateRatio: readI128LE(view, 112) },
    },
  };
}

function decodeSupplyVault(
  data: Uint8Array,
  view: DataView,
  decodeKey: DecodeKeyFn,
): SupplyVault {
  // Layout: mint(32) + mint_decimals(8) + vault(32) + oracle_config(264) + supply_shares(32) + borrow_shares(32) + ...
  const baseOffset = 0;
  return {
    mint: readPubkey(data, baseOffset, decodeKey),
    mintDecimals: Number(readU64LE(view, baseOffset + 32)),
    vault: readPubkey(data, baseOffset + 40, decodeKey),
    oracleConfig: {
      oracleProvider: { kind: "pyth", feedId: new Uint8Array(32), programId: readPubkey(data, baseOffset + 72, decodeKey) },
      validationConfig: { maxStalenessSeconds: 0, maxConfidenceToRateRatio: readI128LE(view, baseOffset + 104) },
    },
    supplySharesTracker: readSharesTracker(view, baseOffset + 336),
    borrowSharesTracker: readSharesTracker(view, baseOffset + 368),
    interestRateCurve: { kind: "adaptive", rateAtTarget: readI128LE(view, baseOffset + 400) },
    lastBorrowInterestRate: { ratePerSecond: readI128LE(view, baseOffset + 528) },
    lastUpdateUnixTimestamp: Number(readI64LE(view, baseOffset + 544)),
    pendingProtocolFeeShares: readU128LE(view, baseOffset + 552),
    pendingCuratorFeeShares: readU128LE(view, baseOffset + 568),
  };
}
