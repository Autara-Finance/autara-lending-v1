/**
 * Fixed-point arithmetic types mirroring autara-lib/src/math/ufixed_point.rs and ifixed_point.rs.
 *
 * The Rust types use U64F64 / I64F64 from the `fixed` crate — 128-bit with 64 fractional bits.
 * In TypeScript we use bigint with an explicit scale factor of 2^64 for faithful representation.
 */

/** 2^64 — matches the 64 fractional bits of U64F64 / I64F64 in Rust */
export const FIXED_POINT_SCALE = 1n << 64n;

declare const __ufixed: unique symbol;
export type UFixedPoint = bigint & { readonly [__ufixed]: true };

declare const __ifixed: unique symbol;
export type IFixedPoint = bigint & { readonly [__ifixed]: true };

export const UFixed = {
  zero(): UFixedPoint {
    return 0n as UFixedPoint;
  },

  one(): UFixedPoint {
    return FIXED_POINT_SCALE as UFixedPoint;
  },

  fromU64(value: bigint): UFixedPoint {
    return (value * FIXED_POINT_SCALE) as UFixedPoint;
  },

  /** Convert from a float — only for display/config, not for on-chain math */
  fromFloat(f: number): UFixedPoint {
    return BigInt(Math.round(f * Number(FIXED_POINT_SCALE))) as unknown as UFixedPoint;
  },

  toFloat(fp: UFixedPoint): number {
    return Number(fp) / Number(FIXED_POINT_SCALE);
  },

  /** Rounded-down integer extraction (mirrors as_u64_rounded_down) */
  toU64RoundedDown(fp: UFixedPoint): bigint {
    return BigInt(fp) / FIXED_POINT_SCALE;
  },

  /** Rounded-up integer extraction (mirrors as_u64_rounded_up) */
  toU64RoundedUp(fp: UFixedPoint): bigint {
    return (BigInt(fp) + FIXED_POINT_SCALE - 1n) / FIXED_POINT_SCALE;
  },

  /** Mirrors UFixedPoint::from_u64_u64_ratio */
  fromU64Ratio(num: bigint, den: bigint): UFixedPoint {
    if (den === 0n) throw new Error("Division by zero in UFixed.fromU64Ratio");
    return ((num * FIXED_POINT_SCALE) / den) as UFixedPoint;
  },

  mul(a: UFixedPoint, b: UFixedPoint): UFixedPoint {
    return ((BigInt(a) * BigInt(b)) / FIXED_POINT_SCALE) as UFixedPoint;
  },

  div(a: UFixedPoint, b: UFixedPoint): UFixedPoint {
    if (BigInt(b) === 0n) throw new Error("Division by zero in UFixed.div");
    return ((BigInt(a) * FIXED_POINT_SCALE) / BigInt(b)) as UFixedPoint;
  },
} as const;

export const IFixed = {
  zero(): IFixedPoint {
    return 0n as IFixedPoint;
  },

  one(): IFixedPoint {
    return FIXED_POINT_SCALE as IFixedPoint;
  },

  fromI64(value: bigint): IFixedPoint {
    return (value * FIXED_POINT_SCALE) as IFixedPoint;
  },

  fromFloat(f: number): IFixedPoint {
    return BigInt(Math.round(f * Number(FIXED_POINT_SCALE))) as unknown as IFixedPoint;
  },

  toFloat(fp: IFixedPoint): number {
    return Number(fp) / Number(FIXED_POINT_SCALE);
  },

  /** Create a ratio from two bigints: (num / den) in fixed-point */
  ratio(num: bigint, den: bigint): IFixedPoint {
    if (den === 0n) throw new Error("Division by zero in IFixed.ratio");
    return ((num * FIXED_POINT_SCALE) / den) as IFixedPoint;
  },

  mul(a: IFixedPoint, b: IFixedPoint): IFixedPoint {
    return ((BigInt(a) * BigInt(b)) / FIXED_POINT_SCALE) as IFixedPoint;
  },

  div(a: IFixedPoint, b: IFixedPoint): IFixedPoint {
    if (BigInt(b) === 0n) throw new Error("Division by zero in IFixed.div");
    return ((BigInt(a) * FIXED_POINT_SCALE) / BigInt(b)) as IFixedPoint;
  },

  gte(a: IFixedPoint, b: IFixedPoint): boolean {
    return BigInt(a) >= BigInt(b);
  },

  gt(a: IFixedPoint, b: IFixedPoint): boolean {
    return BigInt(a) > BigInt(b);
  },
} as const;
