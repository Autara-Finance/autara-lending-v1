import type { PublicKeyStr } from "../domain/types/common";
import { FIXED_POINT_SCALE } from "../domain/types/fixed-point";
import { apyFromRatePerSecond } from "../domain/types/interest-rate";
import type { Market, MarketView } from "../domain/market/market";
import type { Result } from "../domain/result";
import { Result as R } from "../domain/result";
import type { IProtocolReader } from "../gateway/interfaces/protocol-reader";

/**
 * Market service interface — read-only market data and calculations.
 */
export interface IMarketService {
  getMarketState(address: PublicKeyStr): Promise<Result<MarketView | null>>;
  getAllMarkets(): Promise<Result<MarketView[]>>;
  calculateUtilization(market: Market): number;
  getInterestRates(market: MarketView): { borrowApy: number; supplyApy: number };
}

/**
 * Market service implementation.
 * Single Responsibility: only reads market state and computes derived data.
 * No mutations, no wallet dependency.
 */
export class MarketService implements IMarketService {
  constructor(private readonly reader: IProtocolReader) {}

  async getMarketState(
    address: PublicKeyStr,
  ): Promise<Result<MarketView | null>> {
    return R.fromPromise(this.reader.getMarketView(address));
  }

  async getAllMarkets(): Promise<Result<MarketView[]>> {
    return R.fromPromise(
      (async () => {
        const markets = await this.reader.getAllMarkets();
        const views = await Promise.all(
          markets.map((m) => this.reader.getMarketView(m.address)),
        );
        return views.filter((v): v is MarketView => v !== null);
      })(),
    );
  }

  /**
   * Calculate utilization rate for a market.
   * Mirrors SupplyVault::utilisation_rate at supply_vault.rs:105.
   *
   * utilization = total_borrowed / total_supplied
   */
  calculateUtilization(market: Market): number {
    const supplyTracker = market.supplyVault.supplySharesTracker;
    const borrowTracker = market.supplyVault.borrowSharesTracker;

    const totalSupply =
      (BigInt(supplyTracker.totalShares) * BigInt(supplyTracker.atomsPerShare)) /
      FIXED_POINT_SCALE;
    const totalBorrow =
      (BigInt(borrowTracker.totalShares) * BigInt(borrowTracker.atomsPerShare)) /
      FIXED_POINT_SCALE;

    if (totalSupply === 0n) return 0;
    return Number(totalBorrow) / Number(totalSupply);
  }

  /**
   * Get annualized interest rates for a market.
   */
  getInterestRates(market: MarketView): {
    borrowApy: number;
    supplyApy: number;
  } {
    return {
      borrowApy: apyFromRatePerSecond(market.summary.borrowInterestRate),
      supplyApy: apyFromRatePerSecond(market.summary.lendingInterestRate),
    };
  }
}
