import type { PublicKeyStr } from "../domain/types/common";
import type { MarketView } from "../domain/market/market";
import type { IMarketService } from "../services/market-service";

type StateListener<T> = (state: T) => void;

/**
 * Framework-agnostic market state container with polling.
 *
 * This is NOT React state. It's a pure business-layer store that can be
 * subscribed to from React hooks, Svelte stores, or vanilla JS.
 *
 * - Maintains a local cache of all market views
 * - Polls the protocol at a configurable interval
 * - Notifies subscribers on state changes
 */
export class MarketStore {
  private markets = new Map<PublicKeyStr, MarketView>();
  private listeners = new Set<StateListener<ReadonlyMap<PublicKeyStr, MarketView>>>();
  private refreshInterval: ReturnType<typeof setInterval> | null = null;

  constructor(
    private readonly marketService: IMarketService,
    private readonly pollIntervalMs: number = 10_000,
  ) {}

  /** Subscribe to market state changes. Returns an unsubscribe function. */
  subscribe(
    listener: StateListener<ReadonlyMap<PublicKeyStr, MarketView>>,
  ): () => void {
    this.listeners.add(listener);
    // Deliver current state immediately
    listener(this.markets);
    return () => {
      this.listeners.delete(listener);
    };
  }

  /** Get a single market by address */
  getMarket(address: PublicKeyStr): MarketView | undefined {
    return this.markets.get(address);
  }

  /** Get all cached markets */
  getAllMarkets(): ReadonlyMap<PublicKeyStr, MarketView> {
    return this.markets;
  }

  /** Force a refresh of all market data */
  async refresh(): Promise<void> {
    const result = await this.marketService.getAllMarkets();
    if (result.ok) {
      this.markets = new Map(result.value.map((m) => [m.address, m]));
      this.notify();
    }
  }

  /** Start polling for market updates */
  startPolling(): void {
    this.refresh();
    this.refreshInterval = setInterval(
      () => this.refresh(),
      this.pollIntervalMs,
    );
  }

  /** Stop polling */
  stopPolling(): void {
    if (this.refreshInterval) {
      clearInterval(this.refreshInterval);
      this.refreshInterval = null;
    }
  }

  private notify(): void {
    for (const listener of this.listeners) {
      listener(this.markets);
    }
  }
}
