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

    #[test]
    fn lower_bound_less_than_upper_bound() {
        let oracle = OracleRate::new(IFixedPoint::lit("100"), IFixedPoint::lit("5"));
        let lower = oracle.lower_bound_rate().unwrap();
        let upper = oracle.upper_bound_rate().unwrap();
        assert!(lower < upper);
        assert_eq!(lower, IFixedPoint::lit("95"));
        assert_eq!(upper, IFixedPoint::lit("105"));
    }

    #[test]
    fn collateral_value_uses_lower_bound() {
        let oracle = OracleRate::new(IFixedPoint::lit("100"), IFixedPoint::lit("10"));
        let amount = 1_000_000_000u64;
        let value = oracle.collateral_value(amount, 9).unwrap();
        let expected = oracle
            .lower_bound_rate()
            .unwrap()
            .safe_mul(amount)
            .unwrap()
            .safe_div(IFixedPoint::from(1_000_000_000u64))
            .unwrap();
        assert_eq!(value, expected);
    }

    #[test]
    fn borrow_value_uses_upper_bound() {
        let oracle = OracleRate::new(IFixedPoint::lit("100"), IFixedPoint::lit("10"));
        let amount = 1_000_000_000u64;
        let value = oracle.borrow_value(amount, 9).unwrap();
        let expected = oracle
            .upper_bound_rate()
            .unwrap()
            .safe_mul(amount)
            .unwrap()
            .safe_div(IFixedPoint::from(1_000_000_000u64))
            .unwrap();
        assert_eq!(value, expected);
    }

    #[test]
    fn borrow_value_greater_than_collateral_value() {
        let oracle = OracleRate::new(IFixedPoint::lit("100"), IFixedPoint::lit("5"));
        let amount = 1_000_000_000u64;
        let borrow = oracle.borrow_value(amount, 9).unwrap();
        let collateral = oracle.collateral_value(amount, 9).unwrap();
        assert!(borrow > collateral);
    }

    #[test]
    fn value_atoms_roundtrip_borrow() {
        let oracle = OracleRate::new(IFixedPoint::lit("100"), IFixedPoint::lit("1"));
        let atoms = 1_000_000_000u64;
        let value = oracle.borrow_value(atoms, 9).unwrap();
        let recovered_atoms = oracle.borrow_atoms(value, 9).unwrap();
        assert_eq!(recovered_atoms.as_u64_rounded_down().unwrap(), atoms);
    }

    #[test]
    fn value_atoms_roundtrip_collateral() {
        let oracle = OracleRate::new(IFixedPoint::lit("100"), IFixedPoint::lit("1"));
        let atoms = 1_000_000_000u64;
        let value = oracle.collateral_value(atoms, 9).unwrap();
        let recovered_atoms = oracle.collateral_atoms(value, 9).unwrap();
        assert_eq!(recovered_atoms.as_u64_rounded_down().unwrap(), atoms);
    }

    #[test]
    fn relative_confidence_percentage() {
        let oracle = OracleRate::new(IFixedPoint::lit("100"), IFixedPoint::lit("5"));
        let rel_conf = oracle.relative_confidence().unwrap();
        let expected = IFixedPoint::lit("0.05");
        let diff = if rel_conf > expected {
            rel_conf.safe_sub(expected).unwrap()
        } else {
            expected.safe_sub(rel_conf).unwrap()
        };
        assert!(diff < IFixedPoint::lit("0.0001"));
    }

    #[test]
    fn zero_confidence_bounds_equal() {
        let oracle = OracleRate::new(IFixedPoint::lit("100"), IFixedPoint::lit("0"));
        let lower = oracle.lower_bound_rate().unwrap();
        let upper = oracle.upper_bound_rate().unwrap();
        assert_eq!(lower, upper);
        assert_eq!(lower, oracle.rate());
    }

    #[test]
    fn try_from_price_expo_positive() {
        let oracle = OracleRate::try_from_price_expo_conf(100, 5, 2).unwrap();
        assert_eq!(oracle.rate(), IFixedPoint::from(10000u64));
        assert_eq!(oracle.confidence(), IFixedPoint::from(500u64));
    }

    #[test]
    fn try_from_price_expo_negative() {
        let oracle = OracleRate::try_from_price_expo_conf(100_000_000, 1_000_000, -6).unwrap();
        assert_eq!(oracle.rate(), IFixedPoint::lit("100"));
        assert_eq!(oracle.confidence(), IFixedPoint::lit("1"));
    }
}
