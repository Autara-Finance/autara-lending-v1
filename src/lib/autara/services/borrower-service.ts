import type { Atoms } from "../domain/types/atoms";
import { atoms } from "../domain/types/atoms";
import type { PublicKeyStr } from "../domain/types/common";
import { FIXED_POINT_SCALE, IFixed } from "../domain/types/fixed-point";
import type { OracleRate } from "../domain/types/oracle";
import { apyFromRatePerSecond } from "../domain/types/interest-rate";
import type { BorrowPosition } from "../domain/borrower/borrow-position";
import type { BorrowPositionHealth } from "../domain/borrower/borrow-health";
import { assessHealth } from "../domain/borrower/borrow-health";
import type {
  DepositCollateralParams,
  WithdrawCollateralParams,
  BorrowParams,
  RepayParams,
  LeverageParams,
  DeleverageParams,
  BorrowerPositionView,
  LiquidateParams,
} from "../domain/borrower/borrower-types";
import type { MarketView } from "../domain/market/market";
import type { Result } from "../domain/result";
import { Result as R } from "../domain/result";
import { AutaraError, AutaraErrorCode } from "../domain/errors";
import type { TransactionReceipt } from "../gateway/interfaces/transaction-sender";
import type { IProtocolReader } from "../gateway/interfaces/protocol-reader";
import type { ITransactionBuilder } from "../gateway/interfaces/transaction-builder";
import type { ITransactionSender } from "../gateway/interfaces/transaction-sender";
import type { TransactionLifecycle } from "./callbacks";
import { executeTransaction } from "./callbacks";

/**
 * Borrower service interface — Single Responsibility: only borrow-side operations.
 */
export interface IBorrowerService {
  getPosition(
    market: PublicKeyStr,
    authority: PublicKeyStr,
  ): Promise<Result<BorrowerPositionView | null>>;

  getHealth(
    market: PublicKeyStr,
    authority: PublicKeyStr,
  ): Promise<Result<BorrowPositionHealth | null>>;

  depositCollateral(
    params: DepositCollateralParams,
    authority: PublicKeyStr,
    lifecycle?: TransactionLifecycle,
  ): Promise<Result<TransactionReceipt>>;

  withdrawCollateral(
    params: WithdrawCollateralParams,
    authority: PublicKeyStr,
    lifecycle?: TransactionLifecycle,
  ): Promise<Result<TransactionReceipt>>;

  borrow(
    params: BorrowParams,
    authority: PublicKeyStr,
    lifecycle?: TransactionLifecycle,
  ): Promise<Result<TransactionReceipt>>;

  repay(
    params: RepayParams,
    authority: PublicKeyStr,
    lifecycle?: TransactionLifecycle,
  ): Promise<Result<TransactionReceipt>>;

  leverage(
    params: LeverageParams,
    authority: PublicKeyStr,
    lifecycle?: TransactionLifecycle,
  ): Promise<Result<TransactionReceipt>>;

  deleverage(
    params: DeleverageParams,
    authority: PublicKeyStr,
    lifecycle?: TransactionLifecycle,
  ): Promise<Result<TransactionReceipt>>;

  liquidate(
    params: LiquidateParams,
    authority: PublicKeyStr,
    lifecycle?: TransactionLifecycle,
  ): Promise<Result<TransactionReceipt>>;

  calculateMaxBorrowable(collateralAtoms: Atoms, market: MarketView): Atoms;
}

/**
 * Borrower service implementation.
 *
 * Dependency Inversion: depends on IProtocolReader, ITransactionBuilder, ITransactionSender.
 * Health computation mirrors Market::borrow_position_health at market.rs:87.
 */
export class BorrowerService implements IBorrowerService {
  constructor(
    private readonly reader: IProtocolReader,
    private readonly builder: ITransactionBuilder,
    private readonly sender: ITransactionSender,
  ) {}

  async getPosition(
    market: PublicKeyStr,
    authority: PublicKeyStr,
  ): Promise<Result<BorrowerPositionView | null>> {
    return R.fromPromise(
      (async () => {
        const [position, marketView] = await Promise.all([
          this.reader.getBorrowPosition(market, authority),
          this.reader.getMarketView(market),
        ]);
        if (!position || !marketView) return null;

        const health = this.computeHealth(position, marketView);
        const healthStatus = assessHealth(health, marketView.config.ltvConfig);

        // Current debt = borrowed_shares * borrow_atoms_per_share (rounded UP — borrower owes more)
        const borrowTracker = marketView.supplyVault.borrowSharesTracker;
        const currentDebtRaw =
          (BigInt(position.borrowedShares) * BigInt(borrowTracker.atomsPerShare) +
            FIXED_POINT_SCALE - 1n) / FIXED_POINT_SCALE;
        const currentDebtAtoms = atoms(currentDebtRaw);

        const initialRaw = BigInt(position.initialBorrowedAtoms);
        const accruedRaw = currentDebtRaw > initialRaw ? currentDebtRaw - initialRaw : 0n;
        const accruedInterestAtoms = atoms(accruedRaw);

        const borrowApy = apyFromRatePerSecond(marketView.summary.borrowInterestRate);
        const maxBorrowableAtoms = this.calculateMaxBorrowable(
          position.collateralDepositedAtoms,
          marketView,
        );

        return {
          position,
          market: marketView,
          health,
          healthStatus,
          currentDebtAtoms,
          accruedInterestAtoms,
          borrowApy,
          maxBorrowableAtoms,
        };
      })(),
    );
  }

  async getHealth(
    market: PublicKeyStr,
    authority: PublicKeyStr,
  ): Promise<Result<BorrowPositionHealth | null>> {
    return R.fromPromise(
      (async () => {
        const [position, marketView] = await Promise.all([
          this.reader.getBorrowPosition(market, authority),
          this.reader.getMarketView(market),
        ]);
        if (!position || !marketView) return null;
        return this.computeHealth(position, marketView);
      })(),
    );
  }

  async depositCollateral(
    params: DepositCollateralParams,
    authority: PublicKeyStr,
    lifecycle: TransactionLifecycle = {},
  ): Promise<Result<TransactionReceipt>> {
    // Pre-flight: auto-create borrow position if first interaction
    const position = await this.reader.getBorrowPosition(
      params.marketAddress,
      authority,
    );

    return executeTransaction(
      async () => {
        if (!position) {
          await this.builder.createBorrowPosition(params.marketAddress, authority);
        }
        return this.builder.depositCollateral(
          params.marketAddress,
          authority,
          params.amount,
        );
      },
      this.sender,
      lifecycle,
    );
  }

  async withdrawCollateral(
    params: WithdrawCollateralParams,
    authority: PublicKeyStr,
    lifecycle: TransactionLifecycle = {},
  ): Promise<Result<TransactionReceipt>> {
    return executeTransaction(
      () =>
        this.builder.withdrawCollateral(
          params.marketAddress,
          authority,
          params.amount,
        ),
      this.sender,
      lifecycle,
    );
  }

  async borrow(
    params: BorrowParams,
    authority: PublicKeyStr,
    lifecycle: TransactionLifecycle = {},
  ): Promise<Result<TransactionReceipt>> {
    // Pre-flight: simulate LTV check before hitting the chain
    const marketView = await this.reader.getMarketView(params.marketAddress);
    if (marketView) {
      const position = await this.reader.getBorrowPosition(
        params.marketAddress,
        authority,
      );
      if (position) {
        const health = this.computeHealth(position, marketView);
        const additionalBorrowValue = this.atomsToValue(
          params.amount,
          marketView.supplyVault.mintDecimals,
          marketView.supplyOracleRate,
        );
        const projectedBorrowValue =
          BigInt(health.borrowValue) + additionalBorrowValue;
        const projectedLtv = IFixed.ratio(
          projectedBorrowValue,
          BigInt(health.collateralValue),
        );

        if (IFixed.gte(projectedLtv, marketView.config.ltvConfig.maxLtv)) {
          const error = new AutaraError(
            AutaraErrorCode.MaxLtvReached,
            `Projected LTV ${IFixed.toFloat(projectedLtv).toFixed(4)} exceeds max LTV ${IFixed.toFloat(marketView.config.ltvConfig.maxLtv).toFixed(4)}`,
          );
          lifecycle.onError?.(error);
          lifecycle.onSettled?.();
          return R.err(error);
        }
      }
    }

    return executeTransaction(
      () =>
        this.builder.borrow(params.marketAddress, authority, params.amount),
      this.sender,
      lifecycle,
    );
  }

  async repay(
    params: RepayParams,
    authority: PublicKeyStr,
    lifecycle: TransactionLifecycle = {},
  ): Promise<Result<TransactionReceipt>> {
    return executeTransaction(
      () =>
        this.builder.repay(params.marketAddress, authority, params.amount),
      this.sender,
      lifecycle,
    );
  }

  async leverage(
    params: LeverageParams,
    authority: PublicKeyStr,
    lifecycle: TransactionLifecycle = {},
  ): Promise<Result<TransactionReceipt>> {
    return executeTransaction(
      () =>
        this.builder.borrowAndDeposit(
          params.marketAddress,
          authority,
          params,
        ),
      this.sender,
      lifecycle,
    );
  }

  async deleverage(
    params: DeleverageParams,
    authority: PublicKeyStr,
    lifecycle: TransactionLifecycle = {},
  ): Promise<Result<TransactionReceipt>> {
    return executeTransaction(
      () =>
        this.builder.withdrawAndRepay(
          params.marketAddress,
          authority,
          params,
        ),
      this.sender,
      lifecycle,
    );
  }

  async liquidate(
    params: LiquidateParams,
    authority: PublicKeyStr,
    lifecycle: TransactionLifecycle = {},
  ): Promise<Result<TransactionReceipt>> {
    return executeTransaction(
      () =>
        this.builder.liquidate(
          params.marketAddress,
          authority,
          params.positionOwner,
          params.maxRepayAtoms,
        ),
      this.sender,
      lifecycle,
    );
  }

  /**
   * Calculate maximum borrowable amount given collateral and market conditions.
   *
   * max_borrow_value = collateral_value * max_ltv
   * max_borrow_atoms = max_borrow_value / supply_price * 10^supply_decimals
   */
  calculateMaxBorrowable(collateralAtoms: Atoms, market: MarketView): Atoms {
    const collateralValue = this.atomsToValue(
      collateralAtoms,
      market.collateralVault.mintDecimals,
      market.collateralOracleRate,
    );
    const maxLtv = BigInt(market.config.ltvConfig.maxLtv);
    const maxBorrowValue = (collateralValue * maxLtv) / FIXED_POINT_SCALE;

    const supplyRate = BigInt(market.supplyOracleRate.rate);
    if (supplyRate === 0n) return atoms(0n);

    const supplyDecimals = BigInt(10) ** BigInt(market.supplyVault.mintDecimals);
    return atoms((maxBorrowValue * supplyDecimals) / supplyRate);
  }

  /**
   * Compute borrow position health.
   * Mirrors Market::borrow_position_health at market.rs:87.
   *
   * LTV = borrow_value / collateral_value
   * where values are oracle-priced in a common base unit.
   */
  private computeHealth(
    position: BorrowPosition,
    market: MarketView,
  ): BorrowPositionHealth {
    const borrowTracker = market.supplyVault.borrowSharesTracker;

    // Current debt: borrowed_shares * borrow_atoms_per_share (rounded UP)
    const borrowedAtomsRaw =
      (BigInt(position.borrowedShares) * BigInt(borrowTracker.atomsPerShare) +
        FIXED_POINT_SCALE - 1n) / FIXED_POINT_SCALE;

    const borrowValue = this.atomsToValue(
      atoms(borrowedAtomsRaw),
      market.supplyVault.mintDecimals,
      market.supplyOracleRate,
    );
    const collateralValue = this.atomsToValue(
      position.collateralDepositedAtoms,
      market.collateralVault.mintDecimals,
      market.collateralOracleRate,
    );

    const ltv =
      collateralValue > 0n
        ? IFixed.ratio(borrowValue, collateralValue)
        : IFixed.zero();

    return {
      ltv,
      borrowedAtoms: atoms(borrowedAtomsRaw),
      collateralAtoms: position.collateralDepositedAtoms,
      borrowValue: IFixed.ratio(borrowValue, FIXED_POINT_SCALE),
      collateralValue: IFixed.ratio(collateralValue, FIXED_POINT_SCALE),
    };
  }

  /**
   * Convert token atoms to a value in the oracle's base unit.
   * value = atoms * oracle_rate / 10^decimals
   */
  private atomsToValue(
    a: Atoms,
    decimals: number,
    oracle: OracleRate,
  ): bigint {
    return (BigInt(a) * BigInt(oracle.rate)) / (BigInt(10) ** BigInt(decimals));
  }
}
