import type { PublicKeyStr } from "../domain/types/common";
import type { LenderPositionView } from "../domain/lender/lender-types";
import type { BorrowerPositionView } from "../domain/borrower/borrower-types";
import type { ILenderService } from "../services/lender-service";
import type { IBorrowerService } from "../services/borrower-service";

type StateListener<T> = (state: T) => void;

export interface PositionState {
  readonly lenderPositions: ReadonlyMap<PublicKeyStr, LenderPositionView>;
  readonly borrowerPositions: ReadonlyMap<PublicKeyStr, BorrowerPositionView>;
}

/**
 * Framework-agnostic position state container.
 *
 * Tracks a user's positions across all markets.
 * Pairs with MarketStore — this handles user-specific state while
 * MarketStore handles protocol-wide state.
 */
export class PositionStore {
  private lenderPositions = new Map<PublicKeyStr, LenderPositionView>();
  private borrowerPositions = new Map<PublicKeyStr, BorrowerPositionView>();
  private listeners = new Set<StateListener<PositionState>>();

  constructor(
    private readonly lenderService: ILenderService,
    private readonly borrowerService: IBorrowerService,
  ) {}

  /** Subscribe to position state changes */
  subscribe(listener: StateListener<PositionState>): () => void {
    this.listeners.add(listener);
    listener(this.getState());
    return () => {
      this.listeners.delete(listener);
    };
  }

  getState(): PositionState {
    return {
      lenderPositions: this.lenderPositions,
      borrowerPositions: this.borrowerPositions,
    };
  }

  getLenderPosition(market: PublicKeyStr): LenderPositionView | undefined {
    return this.lenderPositions.get(market);
  }

  getBorrowerPosition(market: PublicKeyStr): BorrowerPositionView | undefined {
    return this.borrowerPositions.get(market);
  }

  /** Refresh all positions for a given user across specified markets */
  async refreshForUser(
    authority: PublicKeyStr,
    marketAddresses: PublicKeyStr[],
  ): Promise<void> {
    const results = await Promise.all(
      marketAddresses.flatMap((market) => [
        this.lenderService.getPosition(market, authority),
        this.borrowerService.getPosition(market, authority),
      ]),
    );

    let changed = false;

    for (let i = 0; i < marketAddresses.length; i++) {
      const market = marketAddresses[i];
      const lenderResult = results[i * 2];
      const borrowerResult = results[i * 2 + 1];

      if (lenderResult.ok && lenderResult.value) {
        this.lenderPositions.set(market, lenderResult.value);
        changed = true;
      }

      if (borrowerResult.ok && borrowerResult.value) {
        this.borrowerPositions.set(market, borrowerResult.value);
        changed = true;
      }
    }

    if (changed) this.notify();
  }

  /** Manually set a lender position (for optimistic updates) */
  setLenderPosition(market: PublicKeyStr, position: LenderPositionView): void {
    this.lenderPositions.set(market, position);
    this.notify();
  }

  /** Manually set a borrower position (for optimistic updates) */
  setBorrowerPosition(market: PublicKeyStr, position: BorrowerPositionView): void {
    this.borrowerPositions.set(market, position);
    this.notify();
  }

  private notify(): void {
    const state = this.getState();
    for (const listener of this.listeners) {
      listener(state);
    }
  }
}
