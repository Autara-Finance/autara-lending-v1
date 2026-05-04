import type { Atoms } from "../types/atoms";
import type { PublicKeyStr } from "../types/common";
import type { OracleConfig } from "../types/oracle";

/**
 * Collateral vault — 536 bytes on-chain.
 * Mirrors CollateralVault at state/collateral_vault.rs.
 *
 * Simple accumulator — no shares mechanism, just tracks total collateral.
 * Only used for borrow-side risk management.
 */
export interface CollateralVault {
  readonly mint: PublicKeyStr;
  readonly mintDecimals: number;
  readonly vault: PublicKeyStr;
  readonly oracleConfig: OracleConfig;
  readonly totalCollateralAtoms: Atoms;
}
