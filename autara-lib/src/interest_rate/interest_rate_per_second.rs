use std::ops::Deref;

use borsh::{BorshDeserialize, BorshSerialize};
use bytemuck::{Pod, Zeroable};

use crate::{
    constant::SECONDS_PER_YEAR,
    error::LendingResult,
    interest_rate::interest_rate::InterestRate,
    math::{ifixed_point::IFixedPoint, safe_math::SafeMath},
};

#[repr(transparent)]
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    Pod,
    Zeroable,
    BorshSerialize,
    BorshDeserialize,
    PartialOrd,
    Ord,
)]
#[cfg_attr(
    feature = "client",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct InterestRatePerSecond(pub IFixedPoint);

impl InterestRatePerSecond {
    pub fn new(rate: IFixedPoint) -> Self {
        InterestRatePerSecond(rate)
    }

    pub fn from_apr(apr: IFixedPoint) -> Self {
        InterestRatePerSecond(apr.safe_div(SECONDS_PER_YEAR).expect("should be non zero"))
    }

    pub const fn const_from_apr(apr: IFixedPoint) -> Self {
        InterestRatePerSecond(IFixedPoint::from_bits(
            apr.bits() / SECONDS_PER_YEAR as i128,
        ))
    }

    pub fn coumpounding_interest_rate_during_elapsed_seconds(
        &self,
        elapsed_seconds: u64,
    ) -> LendingResult<InterestRate> {
        self.0
            .safe_mul(elapsed_seconds)?
            .checked_exp()
            .and_then(|x| x.safe_sub(1))
            .map(InterestRate::new)
    }

    pub fn approximate_from_apy(apy: f64) -> Self {
        InterestRatePerSecond(
            IFixedPoint::from_num((apy + 1.).ln())
                .safe_div(SECONDS_PER_YEAR)
                .expect("should be non zero"),
        )
    }

    pub fn approximate_from_apr(apr: f64) -> Self {
        InterestRatePerSecond(IFixedPoint::from_num(apr / SECONDS_PER_YEAR as f64))
    }

    pub fn approximate_apy(&self) -> LendingResult<f64> {
        let rate = self.0.safe_mul(SECONDS_PER_YEAR)?;
        Ok(rate.checked_to_num::<f64>()?.exp() - 1.)
    }

    pub fn approximate_apr(&self) -> LendingResult<f64> {
        let rate = self.0.safe_mul(SECONDS_PER_YEAR)?;
        Ok(rate.checked_to_num::<f64>()?)
    }

    pub fn adjust_for_utilisation_rate(
        self,
        utilisation_rate: IFixedPoint,
    ) -> LendingResult<InterestRatePerSecond> {
        self.0.safe_mul(utilisation_rate).map(InterestRatePerSecond)
    }
}

impl Deref for InterestRatePerSecond {
    type Target = IFixedPoint;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(test)]
pub mod tests {
    use crate::assert_eq_float;

    use super::*;

    #[test]
    pub fn check_apy() {
        let apy = 0.005;
        let rate = InterestRatePerSecond::approximate_from_apy(apy);
        let calculated_apy = rate.approximate_apy().unwrap();
        assert_eq_float!(calculated_apy, apy, 0.0001);
    }

    #[test]
    pub fn check_coumpounding() {
        let apy = 0.05;
        let rate = InterestRatePerSecond::approximate_from_apy(apy);
        let elapsed_seconds = 100000;
        let interest_rate_during_elapsed = rate
            .coumpounding_interest_rate_during_elapsed_seconds(elapsed_seconds)
            .unwrap();
        let interest_rate_during_elapsed_twice = rate
            .coumpounding_interest_rate_during_elapsed_seconds(2 * elapsed_seconds)
            .unwrap();
        assert!(
            interest_rate_during_elapsed_twice
                > InterestRate::new(interest_rate_during_elapsed.rate().safe_mul(2).unwrap())
        );
        let interest_rate_during_one_year = rate
            .coumpounding_interest_rate_during_elapsed_seconds(SECONDS_PER_YEAR)
            .unwrap();
        assert_eq_float!(interest_rate_during_one_year.rate().to_float(), apy)
    }

    #[test]
    pub fn apr_apy_relationship() {
        // APY > APR for same rate due to compounding
        let apr = 0.10;
        let rate_from_apr = InterestRatePerSecond::approximate_from_apr(apr);
        let rate_from_apy = InterestRatePerSecond::approximate_from_apy(apr);
        // For same nominal rate, APY-based rate should be lower (APY already accounts for compounding)
        assert!(rate_from_apr > rate_from_apy);
    }

    #[test]
    pub fn utilisation_rate_scales_interest() {
        let rate = InterestRatePerSecond::approximate_from_apy(0.10);
        let half_util = IFixedPoint::from_ratio(1, 2).unwrap();
        let adjusted = rate.adjust_for_utilisation_rate(half_util).unwrap();
        // Rate per second is halved at 50% utilization
        assert!(*adjusted < *rate);
        let full_util = IFixedPoint::one();
        let adjusted_full = rate.adjust_for_utilisation_rate(full_util).unwrap();
        assert_eq!(*adjusted_full, *rate);
    }

    #[test]
    pub fn zero_elapsed_returns_zero_interest() {
        let rate = InterestRatePerSecond::approximate_from_apy(0.10);
        let interest = rate
            .coumpounding_interest_rate_during_elapsed_seconds(0)
            .unwrap();
        assert!(interest.rate().is_zero());
    }
}
