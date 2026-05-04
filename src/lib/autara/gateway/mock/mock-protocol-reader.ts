import type { PublicKeyStr } from "../../domain/types/common";
import type { OracleConfig, OracleRate } from "../../domain/types/oracle";
import type { Market, MarketView } from "../../domain/market/market";
import type { SupplyPosition } from "../../domain/lender/supply-position";
import type { BorrowPosition } from "../../domain/borrower/borrow-position";
import type { IProtocolReader } from "../interfaces/protocol-reader";
import type { SupplyVaultSummary } from "../../domain/market/supply-vault";

/**
 * In-memory mock implementation of IProtocolReader.
 * Liskov Substitution: can replace ArchProtocolReader in any context.
 *
 * Pre-load state via the seed* methods, then use as a drop-in replacement.
 */
export class MockProtocolReader implements IProtocolReader {
  private markets = new Map<PublicKeyStr, Market>();
  private supplyPositions = new Map<string, SupplyPosition>();
  private borrowPositions = new Map<string, BorrowPosition>();
  private oracleRates = new Map<string, OracleRate>();
  private timestamp = Math.floor(Date.now() / 1000);

  // ── Seed methods for test setup ──────────────────────────────────────

  seedMarket(market: Market): void {
    this.markets.set(market.address, market);
  }

  seedSupplyPosition(position: SupplyPosition): void {
    this.supplyPositions.set(`${position.market}:${position.authority}`, position);
  }

  seedBorrowPosition(position: BorrowPosition): void {
    this.borrowPositions.set(`${position.market}:${position.authority}`, position);
  }

  seedOracleRate(oracleConfig: OracleConfig, rate: OracleRate): void {
    this.oracleRates.set(JSON.stringify(oracleConfig.oracleProvider), rate);
  }

  setTimestamp(ts: number): void {
    this.timestamp = ts;
  }

  // ── IProtocolReader implementation ───────────────────────────────────

  async getMarket(address: PublicKeyStr): Promise<Market | null> {
    return this.markets.get(address) ?? null;
  }

  async getMarketView(address: PublicKeyStr): Promise<MarketView | null> {
    const market = this.markets.get(address);
    if (!market) return null;

    const supplyOracleRate = await this.getOracleRate(market.supplyVault.oracleConfig);
    const collateralOracleRate = await this.getOracleRate(market.collateralVault.oracleConfig);

    const summary: SupplyVaultSummary = {
      lastUpdateUnixTimestamp: market.supplyVault.lastUpdateUnixTimestamp,
      totalSupply: market.supplyVault.supplySharesTracker.totalShares as unknown as typeof summary.totalSupply,
      totalBorrow: market.supplyVault.borrowSharesTracker.totalShares as unknown as typeof summary.totalBorrow,
      pendingCuratorFeeAtoms: market.supplyVault.pendingCuratorFeeShares as unknown as typeof summary.pendingCuratorFeeAtoms,
      pendingProtocolFeeAtoms: market.supplyVault.pendingProtocolFeeShares as unknown as typeof summary.pendingProtocolFeeAtoms,
      utilisationRate: market.config.maxUtilisationRate,
      borrowInterestRate: market.supplyVault.lastBorrowInterestRate,
      lendingInterestRate: market.supplyVault.lastBorrowInterestRate,
    };

    return {
      ...market,
      supplyOracleRate,
      collateralOracleRate,
      summary,
      isOracleStale: false,
    };
  }

  async getSupplyPosition(
    market: PublicKeyStr,
    authority: PublicKeyStr,
  ): Promise<SupplyPosition | null> {
    return this.supplyPositions.get(`${market}:${authority}`) ?? null;
  }

  async getBorrowPosition(
    market: PublicKeyStr,
    authority: PublicKeyStr,
  ): Promise<BorrowPosition | null> {
    return this.borrowPositions.get(`${market}:${authority}`) ?? null;
  }

  async getAllMarkets(): Promise<Market[]> {
    return Array.from(this.markets.values());
  }

  async getUserSupplyPositions(authority: PublicKeyStr): Promise<SupplyPosition[]> {
    return Array.from(this.supplyPositions.values()).filter(
      (p) => p.authority === authority,
    );
  }

  async getUserBorrowPositions(authority: PublicKeyStr): Promise<BorrowPosition[]> {
    return Array.from(this.borrowPositions.values()).filter(
      (p) => p.authority === authority,
    );
  }

  async getOracleRate(oracleConfig: OracleConfig): Promise<OracleRate> {
    const key = JSON.stringify(oracleConfig.oracleProvider);
    const rate = this.oracleRates.get(key);
    if (!rate) {
      // Return a default 1:1 rate for mocking
      const { IFixed } = await import("../../domain/types/fixed-point");
      return { rate: IFixed.one(), confidence: IFixed.zero() };
    }
    return rate;
  }

  async getCurrentTimestamp(): Promise<number> {
    return this.timestamp;
  }
}
