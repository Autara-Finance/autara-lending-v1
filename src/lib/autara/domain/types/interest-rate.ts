import type { IFixedPoint } from "./fixed-point";
import { IFixed } from "./fixed-point";

/**
 * Interest rate per second — mirrors autara-lib/src/interest_rate/interest_rate_per_second.rs.
 * Stored as an IFixedPoint (I64F64).
 */
export interface InterestRatePerSecond {
  readonly ratePerSecond: IFixedPoint;
}

const SECONDS_PER_YEAR = 365.25 * 24 * 3600;

/**
 * Convert a per-second rate to APY using continuous compounding.
 * APY = e^(rate * seconds_per_year) - 1
 */
export function apyFromRatePerSecond(rate: InterestRatePerSecond): number {
  const rateFloat = IFixed.toFloat(rate.ratePerSecond);
  return Math.expm1(rateFloat * SECONDS_PER_YEAR);
}

/**
 * Convert a per-second rate to APR (simple, non-compounded).
 * APR = rate * seconds_per_year
 */
export function aprFromRatePerSecond(rate: InterestRatePerSecond): number {
  return IFixed.toFloat(rate.ratePerSecond) * SECONDS_PER_YEAR;
}

/**
 * Interest rate curve kinds — mirrors interest_rate/interest_rate_kind.rs.
 */
export type InterestRateCurveKind =
  | { kind: "adaptive"; rateAtTarget: IFixedPoint }
  | { kind: "fixed"; ratePerSecond: IFixedPoint }
  | {
      kind: "polyline";
      breakpoints: ReadonlyArray<{
        utilisationRate: IFixedPoint;
        borrowRate: IFixedPoint;
      }>;
    };
