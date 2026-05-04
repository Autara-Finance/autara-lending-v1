import type { Atoms } from "../types/atoms";
import type { IFixedPoint } from "../types/fixed-point";
import { IFixed } from "../types/fixed-point";
import type { LtvConfig } from "../market/market-config";

/**
 * Health snapshot of a borrow position.
 * Mirrors BorrowPositionHealth at state/borrow_position.rs:134.
 */
export interface BorrowPositionHealth {
  readonly ltv: IFixedPoint;
  readonly borrowedAtoms: Atoms;
  readonly collateralAtoms: Atoms;
  readonly borrowValue: IFixedPoint;
  readonly collateralValue: IFixedPoint;
}

/**
 * Categorized health status for UI and business logic decisions.
 */
export type HealthStatus =
  | { readonly status: "healthy"; readonly ltv: number; readonly maxLtv: number }
  | {
      readonly status: "warning";
      readonly ltv: number;
      readonly unhealthyLtv: number;
      readonly distanceToLiquidation: number;
    }
  | { readonly status: "liquidatable"; readonly ltv: number; readonly unhealthyLtv: number };

/**
 * Assess the health of a borrow position against its market's LTV config.
 *
 * - healthy: ltv < maxLtv — can borrow more
 * - warning: maxLtv <= ltv < unhealthyLtv — cannot borrow more, nearing liquidation
 * - liquidatable: ltv >= unhealthyLtv — can be liquidated
 */
export function assessHealth(
  health: BorrowPositionHealth,
  config: LtvConfig,
): HealthStatus {
  const ltv = IFixed.toFloat(health.ltv);
  const maxLtv = IFixed.toFloat(config.maxLtv);
  const unhealthyLtv = IFixed.toFloat(config.unhealthyLtv);

  if (ltv >= unhealthyLtv) {
    return { status: "liquidatable", ltv, unhealthyLtv };
  }
  if (ltv >= maxLtv) {
    return {
      status: "warning",
      ltv,
      unhealthyLtv,
      distanceToLiquidation: unhealthyLtv - ltv,
    };
  }
  return { status: "healthy", ltv, maxLtv };
}
