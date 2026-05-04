import type { PublicKeyStr } from "../types/common";
import type { OracleRate } from "../types/oracle";
import type { MarketConfig } from "./market-config";
import type { CollateralVault } from "./collateral-vault";
import type { SupplyVault, SupplyVaultSummary } from "./supply-vault";

/**
 * Market aggregate — 1448 bytes on-chain.
 * Mirrors Market at state/market.rs:30.
 *
 * Each market is an isolated lending pair:
 *   supply token (e.g. USDC) + collateral token (e.g. SOL)
 */
export interface Market {
  readonly address: PublicKeyStr;
  readonly config: MarketConfig;
  readonly collateralVault: CollateralVault;
  readonly supplyVault: SupplyVault;
}

/**
 * Enriched market view combining on-chain state with live oracle prices.
 * Used by services for calculations that need real-time pricing.
 */
export interface MarketView extends Market {
  readonly supplyOracleRate: OracleRate;
  readonly collateralOracleRate: OracleRate;
  readonly summary: SupplyVaultSummary;
  readonly isOracleStale: boolean;
}
