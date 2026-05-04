import type { Atoms } from "../types/atoms";
import type { PublicKeyStr } from "../types/common";
import type { SupplyPosition } from "./supply-position";
import type { MarketView } from "../market/market";

/**
 * Parameters for supplying tokens to a market.
 */
export interface SupplyParams {
  readonly marketAddress: PublicKeyStr;
  readonly amount: Atoms;
}

/**
 * Parameters for withdrawing supply.
 * Pass "all" to withdraw the entire position.
 */
export interface WithdrawParams {
  readonly marketAddress: PublicKeyStr;
  readonly amount: Atoms | "all";
}

/**
 * Yield calculation result for a lender position.
 */
export interface LenderYieldCalculation {
  readonly currentValueAtoms: Atoms;
  readonly unrealizedYieldAtoms: Atoms;
  readonly yieldPercent: number;
  readonly supplyApy: number;
  readonly projectedYield24h: Atoms;
  readonly projectedYield7d: Atoms;
  readonly projectedYield30d: Atoms;
}

/**
 * Full view of a lender's position with enriched market data.
 */
export interface LenderPositionView {
  readonly position: SupplyPosition;
  readonly market: MarketView;
  readonly currentValueAtoms: Atoms;
  readonly unrealizedYieldAtoms: Atoms;
  readonly yieldPercent: number;
  readonly supplyApy: number;
}
