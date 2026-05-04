/**
 * Branded type for on-chain token amounts.
 * Always bigint, never floating point — mirrors Rust u64 atoms used throughout the protocol.
 *
 * Examples:
 *  - SupplyPosition.deposited_atoms
 *  - CollateralVault.total_collateral_atoms
 *  - BorrowPosition.collateral_deposited_atoms
 */

const U64_MAX = 18_446_744_073_709_551_615n;

declare const __atoms: unique symbol;
export type Atoms = bigint & { readonly [__atoms]: true };

export function atoms(value: bigint): Atoms {
  if (value < 0n) throw new RangeError("Atoms cannot be negative");
  if (value > U64_MAX) throw new RangeError("Atoms exceeds u64::MAX");
  return value as Atoms;
}

export const ZERO_ATOMS = 0n as Atoms;

/**
 * Convert a human-readable decimal string to atoms.
 * Uses string math to avoid floating-point precision loss.
 *
 * @example atomsFromUiAmount("1.5", 6) => 1_500_000n (USDC)
 * @example atomsFromUiAmount("0.001", 9) => 1_000_000n (SOL)
 */
export function atomsFromUiAmount(uiAmount: string, decimals: number): Atoms {
  const [whole = "0", frac = ""] = uiAmount.split(".");
  const padded = frac.padEnd(decimals, "0").slice(0, decimals);
  return atoms(BigInt(whole + padded));
}

/**
 * Convert atoms back to a human-readable decimal string.
 */
export function uiAmountFromAtoms(a: Atoms, decimals: number): string {
  const str = a.toString().padStart(decimals + 1, "0");
  const whole = str.slice(0, str.length - decimals);
  const frac = str.slice(str.length - decimals).replace(/0+$/, "");
  return frac ? `${whole}.${frac}` : whole;
}
