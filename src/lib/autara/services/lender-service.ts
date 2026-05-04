import type { Atoms } from "../domain/types/atoms";
import { atoms } from "../domain/types/atoms";
import type { PublicKeyStr } from "../domain/types/common";
import type { UFixedPoint } from "../domain/types/fixed-point";
import { FIXED_POINT_SCALE } from "../domain/types/fixed-point";
import { apyFromRatePerSecond } from "../domain/types/interest-rate";
import type { SupplyPosition } from "../domain/lender/supply-position";
import type {
  SupplyParams,
  WithdrawParams,
  LenderYieldCalculation,
  LenderPositionView,
} from "../domain/lender/lender-types";
import type { MarketView } from "../domain/market/market";
import type { Result } from "../domain/result";
import { Result as R } from "../domain/result";
import type { TransactionReceipt } from "../gateway/interfaces/transaction-sender";
import type { IProtocolReader } from "../gateway/interfaces/protocol-reader";
import type { ITransactionBuilder } from "../gateway/interfaces/transaction-builder";
import type { ITransactionSender } from "../gateway/interfaces/transaction-sender";
import type { TransactionLifecycle } from "./callbacks";
import { executeTransaction } from "./callbacks";

/**
 * Lender service interface — Single Responsibility: only supply-side operations.
 */
export interface ILenderService {
  getPosition(
    market: PublicKeyStr,
    authority: PublicKeyStr,
  ): Promise<Result<LenderPositionView | null>>;

  supply(
    params: SupplyParams,
    authority: PublicKeyStr,
    lifecycle?: TransactionLifecycle,
  ): Promise<Result<TransactionReceipt>>;

  withdraw(
    params: WithdrawParams,
    authority: PublicKeyStr,
    lifecycle?: TransactionLifecycle,
  ): Promise<Result<TransactionReceipt>>;

  calculateYield(
    position: SupplyPosition,
    market: MarketView,
  ): LenderYieldCalculation;
}

/**
 * Lender service implementation.
 *
 * Dependency Inversion: depends on IProtocolReader, ITransactionBuilder, ITransactionSender.
 * Never touches RPC directly or knows about wallet internals.
 */
export class LenderService implements ILenderService {
  constructor(
    private readonly reader: IProtocolReader,
    private readonly builder: ITransactionBuilder,
    private readonly sender: ITransactionSender,
  ) {}

  async getPosition(
    market: PublicKeyStr,
    authority: PublicKeyStr,
  ): Promise<Result<LenderPositionView | null>> {
    return R.fromPromise(
      (async () => {
        const [position, marketView] = await Promise.all([
          this.reader.getSupplyPosition(market, authority),
          this.reader.getMarketView(market),
        ]);

        if (!position || !marketView) return null;

        const yieldCalc = this.calculateYield(position, marketView);

        return {
          position,
          market: marketView,
          currentValueAtoms: yieldCalc.currentValueAtoms,
          unrealizedYieldAtoms: yieldCalc.unrealizedYieldAtoms,
          yieldPercent: yieldCalc.yieldPercent,
          supplyApy: yieldCalc.supplyApy,
        };
      })(),
    );
  }

  async supply(
    params: SupplyParams,
    authority: PublicKeyStr,
    lifecycle: TransactionLifecycle = {},
  ): Promise<Result<TransactionReceipt>> {
    // Pre-flight: check if supply position exists, auto-create if needed
    const position = await this.reader.getSupplyPosition(
      params.marketAddress,
      authority,
    );

    return executeTransaction(
      async () => {
        // If no position exists, the builder composes CreateSupplyPosition + SupplyApl
        // into a single atomic transaction (mirrors Rust client batching pattern).
        if (!position) {
          await this.builder.createSupplyPosition(params.marketAddress, authority);
        }
        return this.builder.supply(
          params.marketAddress,
          authority,
          params.amount,
        );
      },
      this.sender,
      lifecycle,
    );
  }

  async withdraw(
    params: WithdrawParams,
    authority: PublicKeyStr,
    lifecycle: TransactionLifecycle = {},
  ): Promise<Result<TransactionReceipt>> {
    return executeTransaction(
      () =>
        this.builder.withdrawSupply(
          params.marketAddress,
          authority,
          params.amount,
        ),
      this.sender,
      lifecycle,
    );
  }

  /**
   * Calculate yield for a lender position.
   *
   * Core math mirrors SharesTracker::shares_to_atoms:
   *   current_value = shares * atoms_per_share
   *   unrealized_yield = current_value - deposited_atoms
   */
  calculateYield(
    position: SupplyPosition,
    market: MarketView,
  ): LenderYieldCalculation {
    const tracker = market.supplyVault.supplySharesTracker;

    // shares * atoms_per_share (mirrors SharesTracker::shares_to_atoms, rounded down)
    const currentValueRaw =
      (BigInt(position.shares) * BigInt(tracker.atomsPerShare)) /
      FIXED_POINT_SCALE;
    const currentValueAtoms = atoms(currentValueRaw);

    const depositedRaw = BigInt(position.depositedAtoms);
    const unrealizedRaw = currentValueRaw > depositedRaw ? currentValueRaw - depositedRaw : 0n;
    const unrealizedYieldAtoms = atoms(unrealizedRaw);

    const yieldPercent =
      depositedRaw > 0n ? Number(unrealizedRaw) / Number(depositedRaw) : 0;

    const supplyApy = apyFromRatePerSecond(market.summary.lendingInterestRate);

    // Projected yields based on current APY (simple linear projection)
    const dailyRate = supplyApy / 365;
    const currentValueNum = Number(currentValueRaw);

    return {
      currentValueAtoms,
      unrealizedYieldAtoms,
      yieldPercent,
      supplyApy,
      projectedYield24h: atoms(BigInt(Math.floor(currentValueNum * dailyRate))),
      projectedYield7d: atoms(BigInt(Math.floor(currentValueNum * dailyRate * 7))),
      projectedYield30d: atoms(BigInt(Math.floor(currentValueNum * dailyRate * 30))),
    };
  }
}
