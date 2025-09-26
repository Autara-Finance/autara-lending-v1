use crate::{
    error::{LendingError, LendingResult},
    interest_rate::interest_rate_per_second::InterestRatePerSecond,
    math::{bps::ONE_IN_BPS, ifixed_point::IFixedPoint, safe_math::SafeMath},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarketBorrowRateParameters<'a> {
    pub utilisation_rate: &'a IFixedPoint,
    pub elapsed_seconds_since_last_update: u64,
}

impl MarketBorrowRateParameters<'_> {
    pub fn utilisation_rate_bps(&self) -> LendingResult<u32> {
        Ok(self
            .utilisation_rate
            .safe_mul(ONE_IN_BPS as u64)?
            .as_u64_rounded_down()?
            .try_into()
            .map_err(|_| LendingError::CastOverflow)?)
    }
}

pub trait LendingInterestRateCurveMut {
    fn borrow_rate_per_second(
        &mut self,
        params: MarketBorrowRateParameters,
    ) -> LendingResult<InterestRatePerSecond>;
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    pub fn check_utilisation_rate_bps() {
        let utilisation_rate = IFixedPoint::lit("0.5");
        let elapsed_seconds_since_last_update = 3600;
        let params = MarketBorrowRateParameters {
            utilisation_rate: &utilisation_rate,
            elapsed_seconds_since_last_update,
        };
        assert_eq!(params.utilisation_rate_bps().unwrap(), 5000);
        let utilisation_rate = IFixedPoint::lit("1000000000000000");
        let params = MarketBorrowRateParameters {
            utilisation_rate: &utilisation_rate,
            elapsed_seconds_since_last_update,
        };
        assert_eq!(
            params.utilisation_rate_bps().unwrap_err(),
            LendingError::CastOverflow
        );
    }
}
