use std::{i128, ops::Neg};

use fixed::types::I80F48;

use crate::{
    error::{ErrorWithContext, LendingError, LendingResult},
    map_context,
    math::ufixed_point::UFixedPoint,
    with_context,
};

use super::{pod::PodI128, safe_math::SafeMath};

crate::define_fixed_point!(IFixedPoint, PodI128, I80F48, i128);

impl IFixedPoint {
    pub const fn from_i64(num: i64) -> Self {
        IFixedPoint(PodI128::from_fixed(I80F48::from_bits(
            (num as i128) << I80F48::FRAC_NBITS,
        )))
    }

    pub const fn from_i64_u64_ratio(num: i64, dem: u64) -> Self {
        let bits = ((num as i128) << 48) / (dem as i128);
        IFixedPoint(PodI128::from_fixed(I80F48::from_bits(bits)))
    }

    pub fn is_negative(self) -> bool {
        self.0.fixed() < 0
    }

    pub fn checked_exp(&self) -> LendingResult<Self> {
        // ln(2^-48) = -33.219280948873624
        const MIN_EXP_ARG: IFixedPoint = IFixedPoint::from_i64_u64_ratio(-3321, 100);
        // ln(2^80) = 55.2624584632449
        const MAX_EXP_ARG: IFixedPoint = IFixedPoint::from_i64_u64_ratio(5526, 100);
        const LN_2: IFixedPoint =
            IFixedPoint::from_i64_u64_ratio(693147180559945309, 1000000000000000000);
        const LN_2_DIV_2: IFixedPoint =
            IFixedPoint::from_i64_u64_ratio(693147180559945309, 2000000000000000000);
        fn exp_7_terms(fixed: IFixedPoint) -> LendingResult<IFixedPoint> {
            // x^0 / 0! + x^1 / 1! +...+ x^n / n!
            let mut term = fixed;
            let mut exp_fixed = fixed.safe_add(IFixedPoint::one())?;
            for n in 2u64..7 {
                term = fixed.safe_mul(term)?.safe_div(n)?;
                exp_fixed = exp_fixed.safe_add(term)?;
            }
            Ok(exp_fixed)
        }
        if self <= &MIN_EXP_ARG {
            return Ok(Self::zero());
        } else if self > &MAX_EXP_ARG {
            return Err(LendingError::InvalidExpArg.into());
        }
        let rounding = if self.is_negative() {
            -LN_2_DIV_2
        } else {
            LN_2_DIV_2
        };
        // self = Q * LN_2 + R
        // where Q is an integer and R is in [-LN_2/2, LN_2/2]
        // exp(self) = exp(Q * LN_2 + R)
        //           = exp(Q * LN_2) * exp(R)
        //           = 2^Q * exp(R)
        let q = self
            .safe_add(rounding)?
            .safe_div(LN_2)?
            .checked_to_num::<i32>()?;
        let r = self.safe_sub(Self::from_i64(q as i64).safe_mul(LN_2)?)?;
        let exp_r = exp_7_terms(r)?;
        match exp_r.checked_shift(q) {
            Some(ok) => Ok(ok),
            None => Err(LendingError::MathOverflow.into()),
        }
    }

    pub fn from_ufixed(value: UFixedPoint) -> Option<Self> {
        Self::from_num_checked(value.to_fixed())
    }
}

impl Neg for IFixedPoint {
    type Output = Self;

    fn neg(self) -> Self::Output {
        IFixedPoint(PodI128::from_fixed(-self.0.fixed()))
    }
}

impl TryFrom<UFixedPoint> for IFixedPoint {
    type Error = ErrorWithContext<LendingError>;

    #[track_caller]
    fn try_from(value: UFixedPoint) -> Result<Self, Self::Error> {
        Self::from_ufixed(value).ok_or_else(with_context!(LendingError::CastOverflow))
    }
}

impl SafeMath<UFixedPoint, IFixedPoint> for IFixedPoint {
    #[track_caller]
    fn safe_add(self, other: UFixedPoint) -> LendingResult<Self> {
        let other =
            Self::from_ufixed(other).ok_or_else(with_context!(LendingError::CastOverflow))?;
        self.safe_add(other)
            .map_err(map_context!(LendingError::AdditionOverflow))
    }

    #[track_caller]
    fn safe_sub(self, other: UFixedPoint) -> LendingResult<Self> {
        let other =
            Self::from_ufixed(other).ok_or_else(with_context!(LendingError::CastOverflow))?;
        self.safe_sub(other)
            .map_err(map_context!(LendingError::SubtractionOverflow))
    }

    #[track_caller]
    fn safe_mul(self, other: UFixedPoint) -> LendingResult<Self> {
        let other =
            Self::from_ufixed(other).ok_or_else(with_context!(LendingError::CastOverflow))?;
        self.safe_mul(other)
            .map_err(map_context!(LendingError::MultiplicationOverflow))
    }

    #[track_caller]
    fn safe_div(self, other: UFixedPoint) -> LendingResult<Self> {
        let other =
            Self::from_ufixed(other).ok_or_else(with_context!(LendingError::CastOverflow))?;
        self.safe_div(other)
            .map_err(map_context!(LendingError::DivisionOverflow))
    }
}

#[cfg(test)]
pub mod tests {
    use crate::assert_eq_float;

    use super::*;

    #[test]
    pub fn fixed_exp() {
        let x = [
            "0", "-0.001", "0.001", "-0.5", "-0.5", "1", "-1", "2", "-2", "-20", "20", "50",
        ];
        for x in x {
            let exp_fixed = IFixedPoint::lit(x).checked_exp().unwrap().to_float();
            let exp_float = x.parse::<f64>().unwrap().exp();
            assert_eq_float!(exp_fixed, exp_float, 0.0001) // max 1bps error
        }
    }

    #[test]
    pub fn check_rounding() {
        let x = IFixedPoint::from_i64(1);
        assert_eq!(x.as_u64_rounded_down().unwrap(), 1);
        assert_eq!(x.as_u64_rounded_up().unwrap(), 1);

        let x = IFixedPoint::lit("0.99");
        assert_eq!(x.as_u64_rounded_down().unwrap(), 0);
        assert_eq!(x.as_u64_rounded_up().unwrap(), 1);

        let x = IFixedPoint::lit("0.000000001");
        assert_eq!(x.as_u64_rounded_down().unwrap(), 0);
        assert_eq!(x.as_u64_rounded_up().unwrap(), 1);

        let x = IFixedPoint::lit("-1");
        assert_eq!(
            x.as_u64_rounded_down().unwrap_err(),
            LendingError::CastOverflow
        );
        assert_eq!(
            x.as_u64_rounded_up().unwrap_err(),
            LendingError::CastOverflow
        );
    }

    #[test]
    pub fn test_from_i64_u64_ratio() {
        let result = IFixedPoint::from_i64_u64_ratio(1, 2);
        assert_eq!(result, IFixedPoint::lit("0.5"));
        let result = IFixedPoint::from_i64_u64_ratio(-1, 2);
        assert_eq!(result, IFixedPoint::lit("-0.5"));
    }

    #[test]
    pub fn test_from_ufixed() {
        let ufixed = UFixedPoint::from_num(1.5);
        let ifixed: IFixedPoint = ufixed.try_into().unwrap();
        assert_eq!(ifixed, IFixedPoint::lit("1.5"));
        assert!(IFixedPoint::try_from(UFixedPoint::MAX).is_ok());
        assert!(IFixedPoint::try_from(UFixedPoint::from_bits(1)).is_ok());
    }

    #[test]
    pub fn test_from_fixed() {
        let fixed = I80F48::from_num(1.5);
        let ifixed = IFixedPoint::from_fixed(fixed);
        assert_eq!(ifixed, IFixedPoint::lit("1.5"));

        let fixed = I80F48::from_num(-2.75);
        let ifixed = IFixedPoint::from_fixed(fixed);
        assert_eq!(ifixed, IFixedPoint::lit("-2.75"));
    }

    #[test]
    pub fn check_math() {
        let a = IFixedPoint::lit("1.5");
        let b = IFixedPoint::lit("2.0");
        assert_eq!(a.safe_add(b).unwrap(), IFixedPoint::lit("3.5"));
        assert_eq!(a.safe_sub(b).unwrap(), IFixedPoint::lit("-0.5"));
        assert_eq!(a.safe_mul(b).unwrap(), IFixedPoint::lit("3.0"));
        assert_eq!(a.safe_div(b).unwrap(), IFixedPoint::lit("0.75"));
        assert_eq!(
            IFixedPoint::MAX.safe_add(IFixedPoint::MAX).unwrap_err(),
            LendingError::AdditionOverflow
        );
        assert_eq!(
            IFixedPoint::MIN.safe_sub(IFixedPoint::MAX).unwrap_err(),
            LendingError::SubtractionOverflow
        );
        assert_eq!(
            IFixedPoint::MAX.safe_mul(IFixedPoint::MAX).unwrap_err(),
            LendingError::MultiplicationOverflow
        );
        assert_eq!(
            IFixedPoint::MAX
                .safe_div(IFixedPoint::from_bits(1))
                .unwrap_err(),
            LendingError::DivisionOverflow
        );
        assert_eq!(
            IFixedPoint::lit("1.0")
                .safe_div(IFixedPoint::zero())
                .unwrap_err(),
            LendingError::DivisionOverflow
        );
    }
}
