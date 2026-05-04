import type { Atoms } from "../types/atoms";
import type { IFixedPoint, UFixedPoint } from "../types/fixed-point";
import type { PublicKeyStr } from "../types/common";
import type { OracleConfig } from "../types/oracle";
import type { SharesTrackerView } from "../types/shares";
import type { InterestRateCurveKind, InterestRatePerSecond } from "../types/interest-rate";

/**
 * Supply vault — 720 bytes on-chain.
 * Mirrors SupplyVault at state/supply_vault.rs:31.
 *
 * Contains dual SharesTrackers:
 *  - supplySharesTracker: tracks lender deposits (earn interest)
 *  - borrowSharesTracker: tracks borrower debt (owe interest)
 */
export interface SupplyVault {
  readonly mint: PublicKeyStr;
  readonly mintDecimals: number;
  readonly vault: PublicKeyStr;
  readonly oracleConfig: OracleConfig;
  readonly supplySharesTracker: SharesTrackerView;
  readonly borrowSharesTracker: SharesTrackerView;
  readonly interestRateCurve: InterestRateCurveKind;
  readonly lastBorrowInterestRate: InterestRatePerSecond;
  readonly lastUpdateUnixTimestamp: number;
  readonly pendingProtocolFeeShares: UFixedPoint;
  readonly pendingCuratorFeeShares: UFixedPoint;
}

/**
 * Enriched summary of supply vault state.
 * Mirrors SupplyVaultSummary at state/supply_vault.rs:293.
 */
export interface SupplyVaultSummary {
  readonly lastUpdateUnixTimestamp: number;
  readonly totalSupply: Atoms;
  readonly totalBorrow: Atoms;
  readonly pendingCuratorFeeAtoms: Atoms;
  readonly pendingProtocolFeeAtoms: Atoms;
  readonly utilisationRate: IFixedPoint;
  readonly borrowInterestRate: InterestRatePerSecond;
  readonly lendingInterestRate: InterestRatePerSecond;
}
