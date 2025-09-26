use crate::{
    error::LendingResult,
    math::{ifixed_point::IFixedPoint, safe_math::SafeMath},
    oracle::oracle_price::OracleRate,
};

#[repr(C)]
#[derive(Default, Debug, Clone, Copy)]
pub struct LiquidationResult {
    pub borrowed_atoms_to_repay: u64,
    pub collateral_atoms_to_liquidate: u64,
}

impl LiquidationResult {
    pub fn adjust_for_max_repay(&mut self, max_repay: u64) {
        if self.borrowed_atoms_to_repay > max_repay {
            self.collateral_atoms_to_liquidate =
                ((self.collateral_atoms_to_liquidate as u128 * max_repay as u128)
                    / self.borrowed_atoms_to_repay as u128) as u64;
            self.borrowed_atoms_to_repay = max_repay;
        }
    }
}

#[repr(C)]
#[derive(Default, Debug, Clone, Copy)]
pub struct LiquidationResultWithBonus {
    pub borrowed_atoms_to_repay: u64,
    pub collateral_atoms_to_liquidate: u64,
    pub collateral_atoms_liquidation_bonus: u64,
}

impl LiquidationResultWithBonus {
    pub fn adjust_for_max_repay(&mut self, max_repay: u64) {
        if self.borrowed_atoms_to_repay > max_repay {
            let adjust_fn = |value: u64| {
                ((value as u128 * max_repay as u128) / self.borrowed_atoms_to_repay as u128) as u64
            };
            self.collateral_atoms_to_liquidate = adjust_fn(self.collateral_atoms_to_liquidate);
            self.collateral_atoms_liquidation_bonus =
                adjust_fn(self.collateral_atoms_liquidation_bonus);
            self.borrowed_atoms_to_repay = max_repay;
        }
    }

    pub fn total_collateral_atoms_to_liquidate(&self) -> LendingResult<u64> {
        self.collateral_atoms_to_liquidate
            .safe_add(self.collateral_atoms_liquidation_bonus)
    }
}

/// Caller should assert `ltv` and `new_ltv_after_liquidation_fee` < 1
pub fn compute_liquidation_with_fee(
    borrowed_atoms: u64,
    borrow_decimals: u8,
    supply_oracle: &OracleRate,
    collateral_atoms: u64,
    collateral_decimals: u8,
    collateral_oracle: &OracleRate,
    new_ltv_after_liquidation_fee: IFixedPoint,
    liquidation_fee: IFixedPoint,
    max_borrowed_atoms_to_repay: u64,
) -> LendingResult<LiquidationResultWithBonus> {
    let one_plus_liquidation_fee = IFixedPoint::one().safe_add(liquidation_fee)?;
    let adjusted_new_ltv = new_ltv_after_liquidation_fee.safe_mul(one_plus_liquidation_fee)?;
    let adjusted_collateral_atoms = collateral_atoms
        .safe_div(one_plus_liquidation_fee)?
        .as_u64_rounded_down()?;
    let mut liquidation_result = compute_liquidation(
        borrowed_atoms,
        borrow_decimals,
        supply_oracle,
        adjusted_collateral_atoms,
        collateral_decimals,
        collateral_oracle,
        adjusted_new_ltv,
    )?;
    liquidation_result.adjust_for_max_repay(max_borrowed_atoms_to_repay.min(borrowed_atoms));
    let mut collateral_atoms_fee = liquidation_result
        .collateral_atoms_to_liquidate
        .safe_mul(liquidation_fee)?
        .as_u64_rounded_down()?;
    let total_collateral_atoms_to_liquidate = liquidation_result
        .collateral_atoms_to_liquidate
        .safe_add(collateral_atoms_fee)?;
    if total_collateral_atoms_to_liquidate > collateral_atoms {
        collateral_atoms_fee =
            collateral_atoms.safe_sub(liquidation_result.collateral_atoms_to_liquidate)?;
    }
    let result = LiquidationResultWithBonus {
        borrowed_atoms_to_repay: liquidation_result.borrowed_atoms_to_repay,
        collateral_atoms_to_liquidate: liquidation_result.collateral_atoms_to_liquidate,
        collateral_atoms_liquidation_bonus: collateral_atoms_fee,
    };
    Ok(result)
}

/// Caller should assert `new_ltv` < `ltv`
///
/// Find `borrowed_atoms_to_repay` and `collateral_atoms_to_liquidate` such that:
///
/// `new_ltv` = (`borrowed_atoms` - `borrowed_atoms_to_repay`) * `supply_oracle` / (`collateral_atoms` - `collateral_atoms_to_liquidate`) * `collateral_oracle`
/// and `borrowed_atoms_to_repay` * `supply_oracle` = `collateral_atoms_to_liquidate` * `collateral_oracle` = `value_to_liquidate`
fn compute_liquidation(
    borrowed_atoms: u64,
    borrow_decimals: u8,
    supply_oracle: &OracleRate,
    collateral_atoms: u64,
    collateral_decimals: u8,
    collateral_oracle: &OracleRate,
    new_ltv: IFixedPoint,
) -> LendingResult<LiquidationResult> {
    let value_to_liquidate = supply_oracle
        .borrow_value(borrowed_atoms, borrow_decimals)?
        .safe_sub(
            collateral_oracle
                .collateral_value(collateral_atoms, collateral_decimals)?
                .safe_mul(new_ltv)?,
        )?
        .safe_div(IFixedPoint::one().safe_sub(new_ltv)?)?;
    let collateral_atoms_to_liquidate =
        collateral_oracle.collateral_atoms(value_to_liquidate, collateral_decimals)?;
    let borrowed_atoms_to_repay =
        supply_oracle.borrow_atoms(value_to_liquidate, borrow_decimals)?;
    Ok(LiquidationResult {
        collateral_atoms_to_liquidate: collateral_atoms_to_liquidate.as_u64_rounded_down()?,
        borrowed_atoms_to_repay: borrowed_atoms_to_repay.as_u64_rounded_up()?,
    })
}

#[cfg(test)]
pub mod tests {
    use std::u64;

    use crate::{
        math::ifixed_point::IFixedPoint,
        oracle::oracle_price::OracleRate,
        state::{
            collateral_vault::tests::{BTC, BTC_DECIMALS},
            supply_vault::tests::{USDC, USDC_DECIMALS},
        },
    };

    use super::*;

    struct LiquidationSetup {
        borrow_decimals: u8,
        supply_oracle: OracleRate,
        collateral_decimals: u8,
        collateral_oracle: OracleRate,
        desired_ltv: IFixedPoint,
    }

    impl LiquidationSetup {
        fn new() -> Self {
            Self {
                borrow_decimals: USDC_DECIMALS as u8,
                supply_oracle: OracleRate::new(IFixedPoint::lit("1"), IFixedPoint::lit("0")),
                collateral_decimals: BTC_DECIMALS as u8,
                collateral_oracle: OracleRate::new(
                    IFixedPoint::lit("100000"),
                    IFixedPoint::lit("0"),
                ),
                desired_ltv: IFixedPoint::lit("0.5"),
            }
        }

        fn calculate_ltv(&self, borrowed_atoms: u64, collateral_atoms: u64) -> IFixedPoint {
            if borrowed_atoms == 0 {
                return IFixedPoint::zero();
            }
            self.supply_oracle
                .borrow_value(borrowed_atoms, self.borrow_decimals)
                .unwrap()
                .safe_div(
                    self.collateral_oracle
                        .collateral_value(collateral_atoms, self.collateral_decimals)
                        .unwrap(),
                )
                .unwrap()
        }

        fn calculate_ltv_after_liquidation(
            &self,
            borrowed_atoms: u64,
            collateral_atoms: u64,
            borrowed_atoms_to_repay: u64,
            collateral_atoms_to_liquidate: u64,
            collateral_atoms_liquidation_fee: u64,
        ) -> IFixedPoint {
            let borrowed_atoms_after_liquidation = borrowed_atoms - borrowed_atoms_to_repay;
            let collateral_atoms_after_liquidation =
                collateral_atoms - collateral_atoms_to_liquidate - collateral_atoms_liquidation_fee;
            self.calculate_ltv(
                borrowed_atoms_after_liquidation,
                collateral_atoms_after_liquidation,
            )
        }

        fn assert_ltv_close_to_target(&self, actual_ltv: IFixedPoint, target_ltv: IFixedPoint) {
            assert!(actual_ltv <= target_ltv);
            assert!(actual_ltv > target_ltv.safe_sub(IFixedPoint::lit("0.000001")).unwrap());
        }
    }

    #[test]
    pub fn check_adjust_for_max_repay() {
        let mut liquidation_result = LiquidationResultWithBonus {
            borrowed_atoms_to_repay: 1000,
            collateral_atoms_to_liquidate: 2000,
            collateral_atoms_liquidation_bonus: 100,
        };
        liquidation_result.adjust_for_max_repay(500);
        assert_eq!(liquidation_result.borrowed_atoms_to_repay, 500);
        assert_eq!(liquidation_result.collateral_atoms_to_liquidate, 1000);
        assert_eq!(liquidation_result.collateral_atoms_liquidation_bonus, 50);
    }

    #[test]
    pub fn check_ltv_post_liquidation() {
        let setup = LiquidationSetup::new();
        let borrowed_atoms = USDC(80000.);
        let collateral_atoms = BTC(1.);
        let LiquidationResult {
            collateral_atoms_to_liquidate,
            borrowed_atoms_to_repay,
        } = compute_liquidation(
            borrowed_atoms,
            setup.borrow_decimals,
            &setup.supply_oracle,
            collateral_atoms,
            setup.collateral_decimals,
            &setup.collateral_oracle,
            setup.desired_ltv,
        )
        .unwrap();
        assert_eq!(collateral_atoms_to_liquidate, 60000000);
        assert_eq!(borrowed_atoms_to_repay, 60000000000);
        assert!(borrowed_atoms_to_repay < borrowed_atoms);
        let ltv_after_liquidation = setup.calculate_ltv_after_liquidation(
            borrowed_atoms,
            collateral_atoms,
            borrowed_atoms_to_repay,
            collateral_atoms_to_liquidate,
            0,
        );
        setup.assert_ltv_close_to_target(ltv_after_liquidation, setup.desired_ltv);
    }

    #[test]
    pub fn check_liquidation_with_fee() {
        let setup = LiquidationSetup::new();
        let borrowed_atoms = USDC(80000.);
        let collateral_atoms = BTC(1.);
        let fee = IFixedPoint::lit("0.1");
        let liquidation_result = compute_liquidation_with_fee(
            borrowed_atoms,
            setup.borrow_decimals,
            &setup.supply_oracle,
            collateral_atoms,
            setup.collateral_decimals,
            &setup.collateral_oracle,
            setup.desired_ltv,
            fee,
            u64::MAX,
        )
        .unwrap();
        let ltv_after_liquidation = setup.calculate_ltv_after_liquidation(
            borrowed_atoms,
            collateral_atoms,
            liquidation_result.borrowed_atoms_to_repay,
            liquidation_result.collateral_atoms_to_liquidate,
            liquidation_result.collateral_atoms_liquidation_bonus,
        );
        setup.assert_ltv_close_to_target(ltv_after_liquidation, setup.desired_ltv);
        assert!(liquidation_result.borrowed_atoms_to_repay < borrowed_atoms);
        assert!(
            liquidation_result
                .total_collateral_atoms_to_liquidate()
                .unwrap()
                < collateral_atoms
        );
    }

    #[test]
    pub fn check_full_liquidation_with_reduced_fee() {
        let setup = LiquidationSetup::new();
        let borrowed_atoms = USDC(90910.);
        let collateral_atoms = BTC(1.);
        let fee = IFixedPoint::lit("0.1");
        let liquidation_result = compute_liquidation_with_fee(
            borrowed_atoms,
            setup.borrow_decimals,
            &setup.supply_oracle,
            collateral_atoms,
            setup.collateral_decimals,
            &setup.collateral_oracle,
            setup.desired_ltv,
            fee,
            u64::MAX,
        )
        .unwrap();
        let ltv_after_liquidation = setup.calculate_ltv_after_liquidation(
            borrowed_atoms,
            collateral_atoms,
            liquidation_result.borrowed_atoms_to_repay,
            liquidation_result.collateral_atoms_to_liquidate,
            liquidation_result.collateral_atoms_liquidation_bonus,
        );
        let liquidation_fee =
            liquidation_result.collateral_atoms_liquidation_bonus as f64 / collateral_atoms as f64;
        assert!(liquidation_fee < fee.to_float() - 0.0001);
        assert!(ltv_after_liquidation.is_zero());
        assert!(liquidation_result.borrowed_atoms_to_repay == borrowed_atoms);
        assert!(
            liquidation_result
                .total_collateral_atoms_to_liquidate()
                .unwrap()
                == collateral_atoms
        );
    }
}
