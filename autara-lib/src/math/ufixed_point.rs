use fixed::types::U64F64;

use crate::{
    error::{ErrorWithContext, LendingError},
    math::{ifixed_point::IFixedPoint, pod::PodU128},
    with_context,
};

crate::define_fixed_point!(UFixedPoint, PodU128, U64F64, u128);

impl UFixedPoint {
    pub const fn from_u64_u64_ratio(num: u64, dem: u64) -> Self {
        let bits = ((num as u128) << Self::FRAC_NBITS) / (dem as u128);
        UFixedPoint(PodU128::from_fixed(U64F64::from_bits(bits)))
    }

    pub fn from_ifixed(value: IFixedPoint) -> Option<Self> {
        Self::from_num_checked(value.to_fixed())
    }
}

impl TryFrom<IFixedPoint> for UFixedPoint {
    type Error = ErrorWithContext<LendingError>;

    #[track_caller]
    fn try_from(value: IFixedPoint) -> Result<Self, Self::Error> {
        Self::from_ifixed(value).ok_or_else(with_context!(LendingError::CastOverflow))
    }
}

#[cfg(test)]
pub mod tests {
    use crate::{error::LendingError, math::safe_math::SafeMath};

    use super::*;

    #[test]
    pub fn check_rounding() {
        let x = UFixedPoint::from_u64(1);
        assert_eq!(x.as_u64_rounded_down().unwrap(), 1);
        assert_eq!(x.as_u64_rounded_up().unwrap(), 1);

        let x = UFixedPoint::lit("0.99");
        assert_eq!(x.as_u64_rounded_down().unwrap(), 0);
        assert_eq!(x.as_u64_rounded_up().unwrap(), 1);

        let x = UFixedPoint::lit("0.000000001");
        assert_eq!(x.as_u64_rounded_down().unwrap(), 0);
        assert_eq!(x.as_u64_rounded_up().unwrap(), 1);
    }

    #[test]
    pub fn test_from_fixed() {
        let fixed = U64F64::from_num(1.5);
        let ifixed = UFixedPoint::from_fixed(fixed);
        assert_eq!(ifixed, UFixedPoint::lit("1.5"));
    }

    #[test]
    pub fn test_from_ifixed() {
        let fixed = IFixedPoint::from_num(1.5);
        let ifixed = UFixedPoint::try_from(fixed).unwrap();
        assert_eq!(ifixed, UFixedPoint::lit("1.5"));
        let negative_fixed = IFixedPoint::from_num(-1.5);
        assert_eq!(
            UFixedPoint::try_from(negative_fixed).unwrap_err(),
            LendingError::CastOverflow
        );
    }

    #[test]
    pub fn check_math() {
        let a = UFixedPoint::lit("1.5");
        let b = UFixedPoint::lit("2.0");
        assert_eq!(a.safe_add(b).unwrap(), UFixedPoint::lit("3.5"));
        assert_eq!(b.safe_sub(a).unwrap(), UFixedPoint::lit("0.5"));
        assert_eq!(a.safe_mul(b).unwrap(), UFixedPoint::lit("3.0"));
        assert_eq!(a.safe_div(b).unwrap(), UFixedPoint::lit("0.75"));
        assert_eq!(
            UFixedPoint::MAX.safe_add(UFixedPoint::MAX).unwrap_err(),
            LendingError::AdditionOverflow
        );
        assert_eq!(
            UFixedPoint::MIN.safe_sub(UFixedPoint::MAX).unwrap_err(),
            LendingError::SubtractionOverflow
        );
        assert_eq!(
            UFixedPoint::MAX.safe_mul(UFixedPoint::MAX).unwrap_err(),
            LendingError::MultiplicationOverflow
        );
        assert_eq!(
            UFixedPoint::MAX
                .safe_div(UFixedPoint::from_bits(1))
                .unwrap_err(),
            LendingError::DivisionOverflow
        );
        assert_eq!(
            UFixedPoint::lit("1.0")
                .safe_div(UFixedPoint::zero())
                .unwrap_err(),
            LendingError::DivisionOverflow
        );
    }
}
