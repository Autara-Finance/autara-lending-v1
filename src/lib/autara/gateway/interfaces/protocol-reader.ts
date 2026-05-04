import type { PublicKeyStr } from "../../domain/types/common";
import type { OracleConfig, OracleRate } from "../../domain/types/oracle";
import type { Market, MarketView } from "../../domain/market/market";
import type { SupplyPosition } from "../../domain/lender/supply-position";
import type { BorrowPosition } from "../../domain/borrower/borrow-position";

/**
 * Read-only protocol gateway — Interface Segregation Principle.
 *
 * Reads on-chain state without any wallet dependency.
 * Safe to use from Server Components and Server Actions.
 *
 * Implementations:
 *  - ArchProtocolReader: reads from Arch Network RPC
 *  - MockProtocolReader: in-memory state for testing
 */
export interface IProtocolReader {
  /** Fetch raw market state by address */
  getMarket(address: PublicKeyStr): Promise<Market | null>;

  /** Fetch enriched market view with live oracle prices */
  getMarketView(address: PublicKeyStr): Promise<MarketView | null>;

  /** Fetch a lender's supply position for a specific market */
  getSupplyPosition(
    market: PublicKeyStr,
    authority: PublicKeyStr,
  ): Promise<SupplyPosition | null>;

  /** Fetch a borrower's position for a specific market */
  getBorrowPosition(
    market: PublicKeyStr,
    authority: PublicKeyStr,
  ): Promise<BorrowPosition | null>;

  /** Fetch all known markets */
  getAllMarkets(): Promise<Market[]>;

  /** Fetch all supply positions owned by a user */
  getUserSupplyPositions(authority: PublicKeyStr): Promise<SupplyPosition[]>;

  /** Fetch all borrow positions owned by a user */
  getUserBorrowPositions(authority: PublicKeyStr): Promise<BorrowPosition[]>;

  /** Fetch a live oracle rate for a given oracle config */
  getOracleRate(oracleConfig: OracleConfig): Promise<OracleRate>;

  /** Get the current on-chain timestamp (slot clock) */
  getCurrentTimestamp(): Promise<number>;
}
