import type { Atoms } from "../types/atoms";
import type { UFixedPoint } from "../types/fixed-point";
import type { PublicKeyStr } from "../types/common";

/**
 * Supply position entity — 216 bytes on-chain.
 * Mirrors SupplyPosition at state/supply_position.rs:21.
 *
 * Represents a lender's stake in a market's supply vault.
 * One position per (market, authority) pair via PDA.
 */
export interface SupplyPosition {
  readonly address: PublicKeyStr;
  readonly authority: PublicKeyStr;
  readonly market: PublicKeyStr;
  readonly depositedAtoms: Atoms;
  readonly shares: UFixedPoint;
}
