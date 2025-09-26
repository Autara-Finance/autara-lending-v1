use crate::{
    error::{LendingError, LendingResult},
    map_context,
    math::ifixed_point::IFixedPoint,
    with_context,
};

pub trait SafeMath<Other = Self, Output = Self>: Sized {
    #[track_caller]
    fn safe_add(self, other: Other) -> LendingResult<Output>;
    #[track_caller]
    fn safe_sub(self, other: Other) -> LendingResult<Output>;
    #[track_caller]
    fn safe_mul(self, other: Other) -> LendingResult<Output>;
    #[track_caller]
    fn safe_div(self, other: Other) -> LendingResult<Output>;
}

impl SafeMath for u64 {
    fn safe_add(self, other: Self) -> LendingResult<Self> {
        self.checked_add(other)
            .ok_or_else(with_context!(LendingError::AdditionOverflow))
    }

    fn safe_sub(self, other: Self) -> LendingResult<Self> {
        self.checked_sub(other)
            .ok_or_else(with_context!(LendingError::SubtractionOverflow))
    }

    fn safe_mul(self, other: Self) -> LendingResult<Self> {
        self.checked_mul(other)
            .ok_or_else(with_context!(LendingError::MultiplicationOverflow))
    }

    fn safe_div(self, other: Self) -> LendingResult<Self> {
        self.checked_div(other)
            .ok_or_else(with_context!(LendingError::DivisionByZero))
    }
}

impl SafeMath<IFixedPoint, IFixedPoint> for u64 {
    fn safe_add(self, other: IFixedPoint) -> LendingResult<IFixedPoint> {
        IFixedPoint::from_u64(self)
            .safe_add(other)
            .map_err(map_context!(LendingError::AdditionOverflow))
    }

    fn safe_sub(self, other: IFixedPoint) -> LendingResult<IFixedPoint> {
        IFixedPoint::from_u64(self)
            .safe_sub(other)
            .map_err(map_context!(LendingError::SubtractionOverflow))
    }

    fn safe_mul(self, other: IFixedPoint) -> LendingResult<IFixedPoint> {
        IFixedPoint::from_u64(self)
            .safe_mul(other)
            .map_err(map_context!(LendingError::MultiplicationOverflow))
    }

    fn safe_div(self, other: IFixedPoint) -> LendingResult<IFixedPoint> {
        IFixedPoint::from_u64(self)
            .safe_div(other)
            .map_err(map_context!(LendingError::DivisionOverflow))
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn test_u64_safe_add() {
        assert_eq!(5u64.safe_add(3), Ok(8));
        assert_eq!(
            u64::MAX.safe_add(1).unwrap_err(),
            LendingError::AdditionOverflow
        );
    }

    #[test]
    fn test_u64_safe_sub() {
        assert_eq!(5u64.safe_sub(3), Ok(2));
        assert_eq!(
            3u64.safe_sub(5).unwrap_err(),
            LendingError::SubtractionOverflow
        );
    }

    #[test]
    fn test_u64_safe_mul() {
        assert_eq!(5u64.safe_mul(3), Ok(15));
        assert_eq!(
            u64::MAX.safe_mul(2).unwrap_err(),
            LendingError::MultiplicationOverflow
        );
    }

    #[test]
    fn test_u64_safe_div() {
        assert_eq!(6u64.safe_div(3), Ok(2));
        assert_eq!(5u64.safe_div(0).unwrap_err(), LendingError::DivisionByZero);
    }

    #[test]
    fn test_u64_safe_add_ifixed() {
        let ifixed = IFixedPoint::from_u64(3);
        assert_eq!(5u64.safe_add(ifixed), Ok(IFixedPoint::from_u64(8)));
        assert_eq!(
            u64::MAX.safe_add(IFixedPoint::MAX).unwrap_err(),
            LendingError::AdditionOverflow
        );
    }

    #[test]
    fn test_u64_safe_sub_ifixed() {
        let ifixed = IFixedPoint::from_u64(3);
        assert_eq!(5u64.safe_sub(ifixed), Ok(IFixedPoint::from_u64(2)));
        assert_eq!(
            5u64.safe_sub(IFixedPoint::MIN).unwrap_err(),
            LendingError::SubtractionOverflow
        );
    }

    #[test]
    fn test_u64_safe_mul_ifixed() {
        let ifixed = IFixedPoint::from_u64(3);
        assert_eq!(5u64.safe_mul(ifixed), Ok(IFixedPoint::from_u64(15)));
        assert_eq!(
            u64::MAX.safe_mul(IFixedPoint::MAX).unwrap_err(),
            LendingError::MultiplicationOverflow
        );
    }

    #[test]
    fn test_u64_safe_div_ifixed() {
        let ifixed = IFixedPoint::from_u64(3);
        assert_eq!(6u64.safe_div(ifixed), Ok(IFixedPoint::from_u64(2)));
        assert_eq!(
            5u64.safe_div(IFixedPoint::zero()).unwrap_err(),
            LendingError::DivisionOverflow
        );
        assert_eq!(
            u64::MAX.safe_div(IFixedPoint::from_bits(1)).unwrap_err(),
            LendingError::DivisionOverflow
        );
    }
}
