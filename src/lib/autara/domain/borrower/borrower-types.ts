import type { Atoms } from "../types/atoms";
import type { PublicKeyStr, AccountMeta } from "../types/common";
import type { BorrowPosition } from "./borrow-position";
import type { BorrowPositionHealth, HealthStatus } from "./borrow-health";
import type { MarketView } from "../market/market";

/**
 * Parameters for depositing collateral into a market.
 */
export interface DepositCollateralParams {
  readonly marketAddress: PublicKeyStr;
  readonly amount: Atoms;
}

/**
 * Parameters for withdrawing collateral.
 * Pass "all" to withdraw the entire collateral balance.
 */
export interface WithdrawCollateralParams {
  readonly marketAddress: PublicKeyStr;
  readonly amount: Atoms | "all";
}

/**
 * Parameters for borrowing from a market.
 */
export interface BorrowParams {
  readonly marketAddress: PublicKeyStr;
  readonly amount: Atoms;
}

/**
 * Parameters for repaying borrowed tokens.
 * Pass "all" to repay the entire debt.
 */
export interface RepayParams {
  readonly marketAddress: PublicKeyStr;
  readonly amount: Atoms | "all";
}

/**
 * Parameters for atomic borrow + deposit (leverage).
 * Maps to BorrowDepositApl instruction with optional ix_callback for swaps.
 */
export interface LeverageParams {
  readonly marketAddress: PublicKeyStr;
  readonly borrowAmount: Atoms;
  readonly depositAmount: Atoms;
  readonly swapCallback?: SwapCallbackConfig;
}

/**
 * Parameters for atomic withdraw + repay (deleverage).
 * Maps to WithdrawRepayApl instruction with optional ix_callback for swaps.
 */
export interface DeleverageParams {
  readonly marketAddress: PublicKeyStr;
  readonly withdrawAmount: Atoms | "all";
  readonly repayAmount: Atoms | "all";
  readonly swapCallback?: SwapCallbackConfig;
}

/**
 * Configuration for an on-chain swap callback instruction.
 * Used within BorrowDepositApl and WithdrawRepayApl compound operations.
 */
export interface SwapCallbackConfig {
  readonly programId: PublicKeyStr;
  readonly data: Uint8Array;
  readonly accounts: ReadonlyArray<AccountMeta>;
}

/**
 * Full view of a borrower's position with enriched data.
 */
export interface BorrowerPositionView {
  readonly position: BorrowPosition;
  readonly market: MarketView;
  readonly health: BorrowPositionHealth;
  readonly healthStatus: HealthStatus;
  readonly currentDebtAtoms: Atoms;
  readonly accruedInterestAtoms: Atoms;
  readonly borrowApy: number;
  readonly maxBorrowableAtoms: Atoms;
}

/**
 * Parameters for liquidating an unhealthy borrow position.
 */
export interface LiquidateParams {
  readonly marketAddress: PublicKeyStr;
  readonly positionOwner: PublicKeyStr;
  readonly maxRepayAtoms: Atoms;
}
