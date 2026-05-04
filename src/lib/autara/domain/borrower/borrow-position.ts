import type { Atoms } from "../types/atoms";
import type { UFixedPoint } from "../types/fixed-point";
import type { PublicKeyStr } from "../types/common";

/**
 * Borrow position entity — 224 bytes on-chain.
 * Mirrors BorrowPosition at state/borrow_position.rs:23.
 *
 * Represents a borrower's debt + collateral in a market.
 * One position per (market, authority) pair via PDA.
 */
export interface BorrowPosition {
  readonly address: PublicKeyStr;
  readonly authority: PublicKeyStr;
  readonly market: PublicKeyStr;
  readonly collateralDepositedAtoms: Atoms;
  readonly initialBorrowedAtoms: Atoms;
  readonly borrowedShares: UFixedPoint;
}
