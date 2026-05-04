// Types
export * from "./types";

// Market domain
export type { Market, MarketView } from "./market/market";
export type { MarketConfig, LtvConfig } from "./market/market-config";
export type { SupplyVault, SupplyVaultSummary } from "./market/supply-vault";
export type { CollateralVault } from "./market/collateral-vault";

// Lender domain
export type { SupplyPosition } from "./lender/supply-position";
export type { SupplyParams, WithdrawParams, LenderYieldCalculation, LenderPositionView } from "./lender/lender-types";

// Borrower domain
export type { BorrowPosition } from "./borrower/borrow-position";
export type { BorrowPositionHealth, HealthStatus } from "./borrower/borrow-health";
export { assessHealth } from "./borrower/borrow-health";
export type {
  DepositCollateralParams,
  WithdrawCollateralParams,
  BorrowParams,
  RepayParams,
  LeverageParams,
  DeleverageParams,
  SwapCallbackConfig,
  BorrowerPositionView,
  LiquidateParams,
} from "./borrower/borrower-types";

// Errors and Result
export { AutaraError, AutaraErrorCode } from "./errors";
export type { Result } from "./result";
export { Result as ResultUtils } from "./result";
