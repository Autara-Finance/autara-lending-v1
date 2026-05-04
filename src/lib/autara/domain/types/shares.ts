import type { Atoms } from "./atoms";
import { atoms } from "./atoms";
import type { UFixedPoint } from "./fixed-point";
import { FIXED_POINT_SCALE, UFixed } from "./fixed-point";

/**
 * Read-only view of a SharesTracker — mirrors autara-lib/src/math/shares_tracker.rs.
 *
 * The on-chain SharesTracker tracks total_shares and atoms_per_share.
 * This is the cToken-like mechanism at the heart of the protocol:
 *   - deposit_atoms: user gets shares = atoms / atoms_per_share
 *   - withdraw_shares: user gets atoms = shares * atoms_per_share
 *   - apply_interest_rate: atoms_per_share increases, existing holders earn yield
 */
export interface SharesTrackerView {
  readonly totalShares: UFixedPoint;
  readonly atomsPerShare: UFixedPoint;
}

export type RoundingMode = "down" | "up";

/**
 * Convert atoms to shares at the current rate.
 * Mirrors SharesTracker::atoms_to_shares (always rounds down for deposits).
 */
export function atomsToShares(
  tracker: SharesTrackerView,
  depositAtoms: Atoms,
): UFixedPoint {
  // shares = atoms / atoms_per_share (in fixed-point: atoms * SCALE / atoms_per_share)
  return UFixed.fromU64Ratio(BigInt(depositAtoms), UFixed.toU64RoundedDown(tracker.atomsPerShare) || 1n);
}

/**
 * Convert shares to atoms at the current rate.
 * Mirrors SharesTracker::shares_to_atoms with configurable rounding.
 *  - Round DOWN for withdrawals (lender gets less — conservative for protocol)
 *  - Round UP for debt calculations (borrower owes more — conservative for protocol)
 */
export function sharesToAtoms(
  tracker: SharesTrackerView,
  shares: UFixedPoint,
  rounding: RoundingMode = "down",
): Atoms {
  const raw = BigInt(shares) * BigInt(tracker.atomsPerShare);
  const result =
    rounding === "up"
      ? (raw + FIXED_POINT_SCALE - 1n) / FIXED_POINT_SCALE
      : raw / FIXED_POINT_SCALE;
  return atoms(result);
}

/**
 * Total atoms tracked by this shares tracker.
 */
export function totalAtoms(tracker: SharesTrackerView): Atoms {
  return sharesToAtoms(tracker, tracker.totalShares, "down");
}
