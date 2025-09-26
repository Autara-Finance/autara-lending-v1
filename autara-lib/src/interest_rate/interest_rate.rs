use bytemuck::{Pod, Zeroable};

use crate::{
    error::LendingResult,
    math::{ifixed_point::IFixedPoint, safe_math::SafeMath},
};

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Pod, Zeroable)]
#[cfg_attr(
    feature = "client",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct InterestRate(IFixedPoint);

impl InterestRate {
    pub const fn new(rate: IFixedPoint) -> Self {
        InterestRate(rate)
    }

    pub fn rate(&self) -> IFixedPoint {
        self.0
    }

    pub fn interest<V>(self, value: V) -> LendingResult<IFixedPoint>
    where
        IFixedPoint: SafeMath<V, IFixedPoint>,
    {
        self.0.safe_mul(value)
    }

    pub fn adjust_for_utilisation_rate(
        self,
        utilisation_rate: IFixedPoint,
    ) -> LendingResult<InterestRate> {
        self.0.safe_mul(utilisation_rate).map(InterestRate)
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::math::ifixed_point::IFixedPoint;

    #[test]
    fn test_interest_rate() {
        let rate = InterestRate::new(IFixedPoint::from_ratio(1, 2).unwrap());
        let value = IFixedPoint::from(1000);
        let interest = rate.interest(value).unwrap();
        assert_eq!(interest, IFixedPoint::from(500));
    }

    #[test]
    fn test_adjust_for_utilisation_rate() {
        let rate = InterestRate::new(IFixedPoint::from_ratio(1, 2).unwrap());
        let utilisation_rate = IFixedPoint::from_ratio(8, 10).unwrap();
        let adjusted_rate = rate.adjust_for_utilisation_rate(utilisation_rate).unwrap();
        assert_eq!(
            adjusted_rate.rate(),
            IFixedPoint::from_ratio(4, 10).unwrap()
        );
    }
}
