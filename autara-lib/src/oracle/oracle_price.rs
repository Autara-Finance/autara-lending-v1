use borsh::{BorshDeserialize, BorshSerialize};
use bytemuck::{Pod, Zeroable};

use crate::{
    constant::POSITIVE_POWER_OF_TEN,
    error::{LendingError, LendingResult},
    math::{ifixed_point::IFixedPoint, safe_math::SafeMath},
};

#[repr(C)]
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Pod, Zeroable, BorshSerialize, BorshDeserialize, Default,
)]
#[cfg_attr(
    feature = "client",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
/// Represents the rate per unit of an asset
///
/// Ex : SOL rate = 123 and confidence = 5 => price ~ 123 +/- 5 $ per SOL
pub struct OracleRate {
    rate: IFixedPoint,
    confidence: IFixedPoint,
}

impl OracleRate {
    pub fn new(rate: IFixedPoint, confidence: IFixedPoint) -> Self {
        Self { rate, confidence }
    }

    pub fn try_from_price_expo_conf(price: u64, confidence: u64, expo: i8) -> LendingResult<Self> {
        let expo_pow =
            POSITIVE_POWER_OF_TEN[expo.checked_abs().ok_or(LendingError::CastOverflow)? as usize];
        let scale = |fixed: IFixedPoint| {
            if expo <= 0 {
                fixed.safe_div(expo_pow)
            } else {
                fixed.safe_mul(expo_pow)
            }
        };
        let rate = scale(IFixedPoint::from(price))?;
        let confidence = scale(IFixedPoint::from(confidence))?;
        Ok(Self { rate, confidence })
    }

    pub fn rate(&self) -> IFixedPoint {
        self.rate
    }

    pub fn confidence(&self) -> IFixedPoint {
        self.confidence
    }

    pub fn lower_bound_rate(&self) -> LendingResult<IFixedPoint> {
        self.rate.safe_sub(self.confidence)
    }

    pub fn upper_bound_rate(&self) -> LendingResult<IFixedPoint> {
        self.rate.safe_add(self.confidence)
    }

    pub fn collateral_value(&self, amount: u64, decimals: u8) -> LendingResult<IFixedPoint> {
        self.lower_bound_rate()?
            .safe_mul(amount)?
            .safe_div(POSITIVE_POWER_OF_TEN[decimals as usize])
    }

    pub fn collateral_atoms(&self, value: IFixedPoint, decimals: u8) -> LendingResult<IFixedPoint> {
        value
            .safe_mul(POSITIVE_POWER_OF_TEN[decimals as usize])?
            .safe_div(self.lower_bound_rate()?)
    }

    pub fn borrow_value(&self, amount: u64, decimals: u8) -> LendingResult<IFixedPoint> {
        self.upper_bound_rate()?
            .safe_mul(amount)?
            .safe_div(POSITIVE_POWER_OF_TEN[decimals as usize])
    }

    pub fn borrow_atoms(&self, value: IFixedPoint, decimals: u8) -> LendingResult<IFixedPoint> {
        value
            .safe_mul(POSITIVE_POWER_OF_TEN[decimals as usize])?
            .safe_div(self.upper_bound_rate()?)
    }

    pub fn relative_confidence(&self) -> LendingResult<IFixedPoint> {
        self.confidence.safe_div(self.rate)
    }
}

impl std::fmt::Display for OracleRate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "OracleRate({:.9} +/- {:.6})", self.rate, self.confidence)
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn check_borrow_value() {
        let oracle = OracleRate::new(IFixedPoint::lit("125"), IFixedPoint::lit("1"));
        let borrow_value = oracle.borrow_value(10 * 10u64.pow(9), 9).unwrap();
        assert_eq!(borrow_value, IFixedPoint::lit("1260"));
        let atoms = oracle.borrow_atoms(borrow_value, 9).unwrap();
        assert_eq!(atoms, 10 * 10u64.pow(9));
    }

    #[test]
    fn check_collateral_value() {
        let oracle = OracleRate::new(IFixedPoint::lit("125"), IFixedPoint::lit("1"));
        let borrow_value = oracle.collateral_value(10 * 10u64.pow(9), 9).unwrap();
        assert_eq!(borrow_value, IFixedPoint::lit("1240"));
        let atoms = oracle.collateral_atoms(borrow_value, 9).unwrap();
        assert_eq!(atoms, 10 * 10u64.pow(9));
    }
}
