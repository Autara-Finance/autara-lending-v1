/**
 * Domain error hierarchy mirroring autara-lib/src/error.rs LendingError enum.
 * Every error has a typed code for exhaustive matching.
 */

export enum AutaraErrorCode {
  // Math errors (discriminants 0-6)
  MathOverflow = "MATH_OVERFLOW",
  AdditionOverflow = "ADDITION_OVERFLOW",
  SubtractionOverflow = "SUBTRACTION_OVERFLOW",
  MultiplicationOverflow = "MULTIPLICATION_OVERFLOW",
  DivisionOverflow = "DIVISION_OVERFLOW",
  DivisionByZero = "DIVISION_BY_ZERO",
  CastOverflow = "CAST_OVERFLOW",

  // Position / market errors (discriminants 7-16)
  MaxLtvReached = "MAX_LTV_REACHED",
  MaxUtilisationRateReached = "MAX_UTILISATION_RATE_REACHED",
  InvalidMarketForPosition = "INVALID_MARKET_FOR_POSITION",
  PositionIsHealthy = "POSITION_IS_HEALTHY",
  MaxSupplyReached = "MAX_SUPPLY_REACHED",
  InvalidLtvConfig = "INVALID_LTV_CONFIG",
  InvalidCurve = "INVALID_CURVE",
  InvalidExpArg = "INVALID_EXP_ARG",
  InvalidMaxUtilisationRate = "INVALID_MAX_UTILISATION_RATE",

  // Liquidation / oracle errors (discriminants 16+)
  InvalidLiquidationLtvShouldDecrease = "INVALID_LIQUIDATION_LTV_SHOULD_DECREASE",
  InvalidPythOracleAccount = "INVALID_PYTH_ORACLE_ACCOUNT",
  InvalidChaosOracleAccount = "INVALID_CHAOS_ORACLE_ACCOUNT",
  InvalidOracleFeedId = "INVALID_ORACLE_FEED_ID",
  FailedToLoadAccount = "FAILED_TO_LOAD_ACCOUNT",
  WithdrawalExceedsReserves = "WITHDRAWAL_EXCEEDS_RESERVES",
  WithdrawalExceedsDeposited = "WITHDRAWAL_EXCEEDS_DEPOSITED",
  RepayExceedsBorrowed = "REPAY_EXCEEDS_BORROWED",
  OracleRateTooOld = "ORACLE_RATE_TOO_OLD",
  OracleConfidenceTooLow = "ORACLE_RATE_RELATIVE_CONFIDENCE_TOO_LOW",
  NegativeOracleRate = "NEGATIVE_ORACLE_RATE",
  OracleRateIsNull = "ORACLE_RATE_IS_NULL",
  OracleConfidenceExceedsRate = "ORACLE_CONFIDENCE_EXCEEDS_RATE",
  LiquidationDidNotMeetRequirements = "LIQUIDATION_DID_NOT_MEET_REQUIREMENTS",
  FeeTooHigh = "FEE_TOO_HIGH",
  SharesOverflow = "SHARES_OVERFLOW",
  InvalidNomination = "INVALID_NOMINATION",
  CantModifySharePriceIfZeroShares = "CANT_MODIFY_SHARE_PRICE_IF_ZERO_SHARES",
  NegativeInterestRate = "NEGATIVE_INTEREST_RATE",
  CannotSocializeDebtForHealthyPosition = "CANNOT_SOCIALIZE_DEBT_FOR_HEALTHY_POSITION",
  UnsupportedMintDecimals = "UNSUPPORTED_MINT_DECIMALS",
  InvalidOracleConfig = "INVALID_ORACLE_CONFIG",

  // Client-side errors (no on-chain equivalent)
  TransactionBuildFailed = "TRANSACTION_BUILD_FAILED",
  TransactionSendFailed = "TRANSACTION_SEND_FAILED",
  TransactionConfirmFailed = "TRANSACTION_CONFIRM_FAILED",
  WalletNotConnected = "WALLET_NOT_CONNECTED",
  InsufficientBalance = "INSUFFICIENT_BALANCE",
  AccountNotFound = "ACCOUNT_NOT_FOUND",
  DeserializationFailed = "DESERIALIZATION_FAILED",
  RpcError = "RPC_ERROR",
}

export class AutaraError extends Error {
  public readonly name = "AutaraError";

  constructor(
    public readonly code: AutaraErrorCode,
    message: string,
    public readonly cause?: unknown,
  ) {
    super(message);
  }

  /**
   * Map a Rust LendingError u8 discriminant to a typed AutaraError.
   * Discriminant order matches the `#[repr(u8)]` enum at error.rs:61.
   */
  static fromOnChainError(discriminant: number): AutaraError {
    const mapping: Record<number, AutaraErrorCode> = {
      0: AutaraErrorCode.MathOverflow,
      1: AutaraErrorCode.AdditionOverflow,
      2: AutaraErrorCode.SubtractionOverflow,
      3: AutaraErrorCode.MultiplicationOverflow,
      4: AutaraErrorCode.DivisionOverflow,
      5: AutaraErrorCode.DivisionByZero,
      6: AutaraErrorCode.CastOverflow,
      7: AutaraErrorCode.MaxLtvReached,
      8: AutaraErrorCode.MaxUtilisationRateReached,
      9: AutaraErrorCode.InvalidMarketForPosition,
      10: AutaraErrorCode.PositionIsHealthy,
      11: AutaraErrorCode.MaxSupplyReached,
      12: AutaraErrorCode.InvalidLtvConfig,
      13: AutaraErrorCode.InvalidCurve,
      14: AutaraErrorCode.InvalidExpArg,
      15: AutaraErrorCode.InvalidMaxUtilisationRate,
      16: AutaraErrorCode.InvalidLiquidationLtvShouldDecrease,
      17: AutaraErrorCode.InvalidPythOracleAccount,
      18: AutaraErrorCode.InvalidChaosOracleAccount,
      19: AutaraErrorCode.InvalidOracleFeedId,
      20: AutaraErrorCode.FailedToLoadAccount,
      21: AutaraErrorCode.WithdrawalExceedsReserves,
      22: AutaraErrorCode.WithdrawalExceedsDeposited,
      23: AutaraErrorCode.RepayExceedsBorrowed,
      24: AutaraErrorCode.OracleRateTooOld,
      25: AutaraErrorCode.OracleConfidenceTooLow,
      26: AutaraErrorCode.NegativeOracleRate,
      27: AutaraErrorCode.OracleRateIsNull,
      28: AutaraErrorCode.OracleConfidenceExceedsRate,
      29: AutaraErrorCode.LiquidationDidNotMeetRequirements,
      30: AutaraErrorCode.FeeTooHigh,
      31: AutaraErrorCode.SharesOverflow,
      32: AutaraErrorCode.InvalidNomination,
      33: AutaraErrorCode.CantModifySharePriceIfZeroShares,
      34: AutaraErrorCode.NegativeInterestRate,
      35: AutaraErrorCode.CannotSocializeDebtForHealthyPosition,
      36: AutaraErrorCode.UnsupportedMintDecimals,
      37: AutaraErrorCode.InvalidOracleConfig,
    };

    const code = mapping[discriminant] ?? AutaraErrorCode.MathOverflow;
    return new AutaraError(code, `On-chain error [${discriminant}]: ${code}`);
  }
}
