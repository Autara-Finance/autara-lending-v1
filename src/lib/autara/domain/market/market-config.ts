import type { IFixedPoint } from "../types/fixed-point";
import type { Atoms } from "../types/atoms";
import type { PublicKeyStr } from "../types/common";

/**
 * LTV configuration for a market.
 * Mirrors LtvConfig at state/market_config.rs:201.
 *
 * Invariants (enforced on-chain):
 *  - max_ltv < unhealthy_ltv (safety buffer, typically 5-10%)
 *  - unhealthy_ltv * (1 + liquidation_bonus) <= 0.99
 *  - liquidation_bonus in [0.001, 0.10]
 */
export interface LtvConfig {
  readonly maxLtv: IFixedPoint;
  readonly unhealthyLtv: IFixedPoint;
  readonly liquidationBonus: IFixedPoint;
}

/**
 * Market configuration — 192 bytes on-chain.
 * Mirrors MarketConfig at state/market_config.rs:26.
 */
export interface MarketConfig {
  readonly bump: number;
  readonly index: number;
  readonly lendingMarketFeeInBps: number;
  readonly protocolFeeShareInBps: number;
  readonly curator: PublicKeyStr;
  readonly ltvConfig: LtvConfig;
  readonly maxUtilisationRate: IFixedPoint;
  readonly maxSupplyAtoms: Atoms;
}
