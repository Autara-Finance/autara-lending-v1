import type { IFixedPoint } from "./fixed-point";
import type { PublicKeyStr } from "./common";

/**
 * Oracle rate with confidence interval.
 * Mirrors OracleRate at oracle/oracle_price.rs.
 *
 * Example: SOL rate = 123, confidence = 5 => price ~ 123 +/- 5 USD per SOL
 */
export interface OracleRate {
  readonly rate: IFixedPoint;
  readonly confidence: IFixedPoint;
}

/**
 * Oracle provider configuration.
 * Mirrors oracle/oracle_provider.rs OracleProvider enum.
 */
export type OracleProvider =
  | { kind: "pyth"; feedId: Uint8Array; programId: PublicKeyStr }
  | { kind: "chaos"; feedId: Uint8Array; programId: PublicKeyStr; requiredSignatures: number };

/**
 * Oracle validation config — staleness and confidence thresholds.
 * Mirrors oracle/oracle_config.rs OracleValidationConfig.
 */
export interface OracleValidationConfig {
  readonly maxStalenessSeconds: number;
  readonly maxConfidenceToRateRatio: IFixedPoint;
}

/**
 * Full oracle configuration for a vault.
 * Mirrors OracleConfig at oracle/oracle_config.rs (264 bytes on-chain).
 */
export interface OracleConfig {
  readonly oracleProvider: OracleProvider;
  readonly validationConfig: OracleValidationConfig;
}
