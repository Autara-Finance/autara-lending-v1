import type { IProtocolReader } from "./gateway/interfaces/protocol-reader";
import type { ITransactionBuilder } from "./gateway/interfaces/transaction-builder";
import type { ITransactionSender } from "./gateway/interfaces/transaction-sender";
import { LenderService, type ILenderService } from "./services/lender-service";
import { BorrowerService, type IBorrowerService } from "./services/borrower-service";
import { MarketService, type IMarketService } from "./services/market-service";
import { MarketStore } from "./state/market-store";
import { PositionStore } from "./state/position-store";

/**
 * All services needed by the application.
 */
export interface AutaraServices {
  readonly reader: IProtocolReader;
  readonly lender: ILenderService;
  readonly borrower: IBorrowerService;
  readonly market: IMarketService;
  readonly marketStore: MarketStore;
  readonly positionStore: PositionStore;
}

export interface AutaraServicesConfig {
  readonly reader: IProtocolReader;
  readonly builder: ITransactionBuilder;
  readonly sender: ITransactionSender;
  readonly pollIntervalMs?: number;
}

/**
 * Composition root — wires all dependencies together.
 *
 * Dependency Inversion: accepts interfaces, not concrete implementations.
 * The caller decides which gateway implementations to inject:
 *   - Production: ArchProtocolReader + ArchTransactionBuilder + WalletTransactionSender
 *   - Testing: MockProtocolReader + MockTransactionBuilder + MockTransactionSender
 */
export function createAutaraServices(config: AutaraServicesConfig): AutaraServices {
  const { reader, builder, sender, pollIntervalMs } = config;

  const market = new MarketService(reader);
  const lender = new LenderService(reader, builder, sender);
  const borrower = new BorrowerService(reader, builder, sender);
  const marketStore = new MarketStore(market, pollIntervalMs);
  const positionStore = new PositionStore(lender, borrower);

  return { reader, lender, borrower, market, marketStore, positionStore };
}
