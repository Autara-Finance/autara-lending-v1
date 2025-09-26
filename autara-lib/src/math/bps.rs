use crate::{
    error::LendingResult,
    math::{safe_math::SafeMath, ufixed_point::UFixedPoint},
};

pub const ONE_IN_BPS: u32 = 10_000;

pub const fn percent_to_bps(percent: u64) -> u64 {
    percent * 100
}

pub const fn bps_to_percent(bps: u64) -> u64 {
    bps / 100
}

pub const fn bps_to_fixed_point(bps: u64) -> UFixedPoint {
    UFixedPoint::from_u64_u64_ratio(bps as _, ONE_IN_BPS as _)
}

pub fn bps_from_fixed_point(bps: u64, fixed: UFixedPoint) -> LendingResult<UFixedPoint> {
    fixed
        .safe_mul(bps)
        .map(|r| r.safe_div(ONE_IN_BPS as u64).expect("should not overflow"))
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn test_percent_to_bps() {
        assert_eq!(percent_to_bps(1), 100);
        assert_eq!(percent_to_bps(10), 1000);
        assert_eq!(percent_to_bps(50), 5000);
    }

    #[test]
    fn test_bps_to_percent() {
        assert_eq!(bps_to_percent(100), 1);
        assert_eq!(bps_to_percent(1000), 10);
        assert_eq!(bps_to_percent(5000), 50);
    }

    #[test]
    fn test_bps_to_fixed_point() {
        assert_eq!(
            bps_to_fixed_point(100),
            UFixedPoint::from_u64_u64_ratio(100, 10_000)
        );
        assert_eq!(
            bps_to_fixed_point(1000),
            UFixedPoint::from_u64_u64_ratio(1000, 10_000)
        );
        assert_eq!(
            bps_to_fixed_point(5000),
            UFixedPoint::from_u64_u64_ratio(5000, 10_000)
        );
    }

    #[test]
    fn test_bps_from_fixed_point() {
        let result = bps_from_fixed_point(1, UFixedPoint::from_u64(2));
        assert_eq!(result, Ok(UFixedPoint::from_u64_u64_ratio(2, 10_000)));
    }
}
