use arch_program::pubkey::Pubkey;
use bytemuck::{Pod, Zeroable};

use crate::{
    interest_rate::interest_rate_kind::InterestRateCurveKind,
    math::{ifixed_point::IFixedPoint, rounding::RoundingMode, safe_math::SafeMath},
    operation::liquidation::{compute_liquidation_with_fee, LiquidationResultWithBonus},
    oracle::{oracle_config::OracleConfig, oracle_price::OracleRate},
    pda::market_seed_with_bump,
    state::{borrow_position::LiquidationResultWithCtx, market_config::MarketConfig},
    token::TokenInfo,
};

use super::{
    super::error::{LendingError, LendingResult, StackTrace},
    borrow_position::{BorrowPosition, BorrowPositionHealth},
    collateral_vault::CollateralVault,
    supply_position::SupplyPosition,
    supply_vault::SupplyVault,
};

crate::validate_struct!(Market, 1432);

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[cfg_attr(
    feature = "client",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct Market {
    config: MarketConfig,
    collateral_vault: CollateralVault,
    supply_vault: SupplyVault,
}

impl Market {
    #[inline(always)]
    pub fn supply_token_info(&self) -> TokenInfo {
        TokenInfo {
            mint: *self.supply_vault.mint(),
            decimals: self.supply_vault.mint_decimals(),
        }
    }

    #[inline(always)]
    pub fn collateral_token_info(&self) -> TokenInfo {
        TokenInfo {
            mint: *self.collateral_vault.mint(),
            decimals: self.collateral_vault.mint_decimals(),
        }
    }

    #[inline(always)]
    pub fn supply_vault(&self) -> &SupplyVault {
        &self.supply_vault
    }

    #[inline(always)]
    pub fn collateral_vault(&self) -> &CollateralVault {
        &self.collateral_vault
    }

    pub fn set_supply_oracle_config(&mut self, oracle_config: OracleConfig) {
        self.supply_vault.set_oracle_config(oracle_config);
    }

    pub fn set_collateral_oracle_config(&mut self, oracle_config: OracleConfig) {
        self.collateral_vault.set_oracle_config(oracle_config);
    }

    #[inline(always)]
    pub fn config(&self) -> &MarketConfig {
        &self.config
    }

    #[inline(always)]
    pub fn seed(&self) -> [&[u8]; 6] {
        market_seed_with_bump(
            self.config.curator(),
            self.supply_vault.mint(),
            self.collateral_vault.mint(),
            self.config.index(),
            self.config.bump(),
        )
    }

    pub fn borrow_position_health(
        &self,
        borrow_position: &BorrowPosition,
        collateral_oracle: &OracleRate,
        supply_oracle: &OracleRate,
    ) -> LendingResult<BorrowPositionHealth> {
        let borrowed_atoms = self
            .supply_vault
            .borrow_shares_to_atoms(borrow_position.borrowed_shares())
            .track_caller()?;
        let borrow_value = supply_oracle
            .borrow_value(borrowed_atoms, self.supply_vault.mint_decimals())
            .track_caller()?;
        let collateral_value = collateral_oracle
            .collateral_value(
                borrow_position.collateral_deposited_atoms(),
                self.collateral_vault.mint_decimals(),
            )
            .track_caller()?;
        let ltv = if borrow_value.is_zero() {
            IFixedPoint::zero()
        } else {
            borrow_value.safe_div(collateral_value).track_caller()?
        };
        Ok(BorrowPositionHealth {
            ltv,
            collateral_atoms: borrow_position.collateral_deposited_atoms(),
            borrowed_atoms,
            borrow_value,
            collateral_value,
        })
    }

    pub fn supply_position_info(&self, supply_position: &SupplyPosition) -> LendingResult<u64> {
        self.supply_vault
            .supply_shares_tracker()
            .shares_to_atoms(supply_position.shares(), RoundingMode::RoundDown)
    }

    pub fn position_checks_on_ltv_increase(
        &self,
        position: &BorrowPositionHealth,
    ) -> LendingResult {
        if position.ltv > self.config.ltv_config().max_ltv {
            return Err(LendingError::MaxLtvReached.into());
        }
        Ok(())
    }

    #[cfg(feature = "client")]
    pub fn get_oracle_keys(&self) -> (Pubkey, Pubkey) {
        (
            self.supply_vault
                .oracle_provider()
                .oracle_provider_ref()
                .autara_pyth_pubkey()
                .unwrap(),
            self.collateral_vault
                .oracle_provider()
                .oracle_provider_ref()
                .autara_pyth_pubkey()
                .unwrap(),
        )
    }

    pub fn initialize_collateral_vault(
        &mut self,
        mint: Pubkey,
        mint_decimals: u64,
        vault: Pubkey,
        oracle_config: OracleConfig,
    ) -> LendingResult {
        self.collateral_vault
            .initialize(mint, mint_decimals, vault, oracle_config)?;
        Ok(())
    }

    pub fn initlize_supply_vault(
        &mut self,
        mint: Pubkey,
        mint_decimals: u64,
        vault: Pubkey,
        oracle_config: OracleConfig,
        interest_rate: InterestRateCurveKind,
        timestamp: i64,
    ) -> LendingResult {
        self.supply_vault.initialize(
            mint,
            mint_decimals,
            vault,
            oracle_config,
            interest_rate,
            timestamp,
        )
    }

    pub fn config_mut(&mut self) -> &mut MarketConfig {
        &mut self.config
    }

    pub fn sync_clock(&mut self, unix_timestamp: i64) -> LendingResult {
        self.supply_vault.sync_clock(
            unix_timestamp,
            self.config.lending_market_fee_fixed(),
            self.config.protocol_fee_in_bps(),
        )
    }

    pub fn redeem_protocol_fees(&mut self) -> LendingResult<u64> {
        let atoms = self.supply_vault.redeem_protocol_fees()?;
        if self.supply_vault.utilisation_rate()? > IFixedPoint::one() {
            return Err(LendingError::WithdrawalExceedsReserves.into());
        }
        Ok(atoms)
    }

    pub fn redeem_curator_fess(&mut self) -> LendingResult<u64> {
        let atoms = self.supply_vault.redeem_curator_fees()?;
        if self.supply_vault.utilisation_rate()? > IFixedPoint::one() {
            return Err(LendingError::WithdrawalExceedsReserves.into());
        }
        Ok(atoms)
    }

    pub fn donate_supply_atoms(&mut self, atoms: u64) -> LendingResult {
        self.supply_vault.donate_supply(atoms)
    }

    pub(super) fn lend(
        &mut self,
        supply_position: &mut SupplyPosition,
        atoms: u64,
    ) -> LendingResult {
        let shares = self.supply_vault.lend(atoms).track_caller()?;
        supply_position.lend(atoms, shares)?;
        if self.supply_vault.total_supply()? > self.config.max_supply_atoms() {
            return Err(LendingError::MaxSupplyReached.into());
        }
        Ok(())
    }

    pub(super) fn withdraw(
        &mut self,
        supply_position: &mut SupplyPosition,
        atoms: u64,
    ) -> LendingResult {
        let shares = self.supply_vault.withdraw_atoms(atoms).track_caller()?;
        if self.supply_vault.utilisation_rate()? > IFixedPoint::one() {
            return Err(LendingError::WithdrawalExceedsReserves.into());
        }
        supply_position.withdraw(shares).track_caller()?;
        Ok(())
    }

    pub(super) fn withdraw_all(
        &mut self,
        supply_position: &mut SupplyPosition,
    ) -> LendingResult<u64> {
        let atoms = self
            .supply_vault
            .withdraw_shares(supply_position.shares())
            .track_caller()?;
        if self.supply_vault.utilisation_rate()? > IFixedPoint::one() {
            return Err(LendingError::WithdrawalExceedsReserves.into());
        }
        supply_position.withdraw_all();
        Ok(atoms)
    }

    pub(super) fn deposit_collateral(
        &mut self,
        borrow_position: &mut BorrowPosition,
        atoms: u64,
    ) -> LendingResult {
        borrow_position.deposit_collateral(atoms)?;
        self.collateral_vault
            .deposit_collateral(atoms)
            .track_caller()?;
        Ok(())
    }

    pub(super) fn withdraw_collateral(
        &mut self,
        borrow_position: &mut BorrowPosition,
        atoms: u64,
        collateral_oracle: &OracleRate,
        supply_oracle: &OracleRate,
    ) -> LendingResult {
        borrow_position.withdraw_collateral(atoms)?;
        // Check if collateral is zero to avoid division by zero in health calculation
        if borrow_position.collateral_deposited_atoms() == 0
            && !borrow_position.borrowed_shares().is_zero()
        {
            return Err(LendingError::MaxLtvReached.into());
        }
        let health = self
            .borrow_position_health(borrow_position, collateral_oracle, supply_oracle)
            .track_caller()?;
        self.collateral_vault
            .withdraw_collateral(atoms)
            .track_caller()?;
        self.position_checks_on_ltv_increase(&health)?;
        Ok(())
    }

    pub(super) fn borrow(
        &mut self,
        borrow_position: &mut BorrowPosition,
        borrow_atoms: u64,
        supply_oracle: &OracleRate,
        collateral_oracle: &OracleRate,
    ) -> LendingResult {
        let shares = self.supply_vault.borrow(borrow_atoms).track_caller()?;
        borrow_position
            .borrow(borrow_atoms, shares)
            .track_caller()?;
        let health = self
            .borrow_position_health(borrow_position, collateral_oracle, supply_oracle)
            .track_caller()?;
        self.position_checks_on_ltv_increase(&health)?;
        if self.supply_vault.utilisation_rate()? > self.config.max_utilisation_rate() {
            return Err(LendingError::MaxUtilisationRateReached.into());
        }
        Ok(())
    }

    pub(super) fn repay(
        &mut self,
        borrow_position: &mut BorrowPosition,
        atoms: u64,
    ) -> LendingResult {
        let shares = self.supply_vault.repay_atoms(atoms).track_caller()?;
        borrow_position.repay(shares).track_caller()?;
        Ok(())
    }

    pub(super) fn repay_all(&mut self, borrow_position: &mut BorrowPosition) -> LendingResult<u64> {
        let atoms = self
            .supply_vault
            .repay_shares(borrow_position.borrowed_shares())
            .track_caller()?;
        borrow_position.repay_all();
        Ok(atoms)
    }

    pub(super) fn liquidate(
        &mut self,
        borrow_position: &mut BorrowPosition,
        collateral_oracle: &OracleRate,
        supply_oracle: &OracleRate,
        max_repay_atoms: u64,
    ) -> LendingResult<LiquidationResultWithCtx> {
        let (health_before, mut liquidation) = self
            .compute_liquidation_result_with_fee(
                borrow_position,
                collateral_oracle,
                supply_oracle,
                max_repay_atoms,
            )
            .track_caller()?;
        let (atoms_repaid, shares_repaid) = self
            .supply_vault
            .repay_atoms_capped(
                liquidation.borrowed_atoms_to_repay,
                borrow_position.borrowed_shares(),
            )
            .track_caller()?;
        // because of rounding we need to adjust the liquidation result
        liquidation.adjust_for_max_repay(atoms_repaid);
        borrow_position.liquidate(
            shares_repaid,
            liquidation.total_collateral_atoms_to_liquidate()?,
        )?;
        let health_after = self
            .borrow_position_health(borrow_position, collateral_oracle, supply_oracle)
            .track_caller()?;
        if health_after.ltv > health_before.ltv {
            return Err(LendingError::InvalidLiquidationLtvShouldDecrease.into());
        }
        Ok(LiquidationResultWithCtx {
            liquidation_result_with_bonus: liquidation,
            health_before_liquidation: health_before,
            health_after_liquidation: health_after,
        })
    }

    pub fn compute_liquidation_result_with_fee(
        &self,
        borrow_position: &BorrowPosition,
        collateral_oracle: &OracleRate,
        supply_oracle: &OracleRate,
        max_repay_atoms: u64,
    ) -> LendingResult<(BorrowPositionHealth, LiquidationResultWithBonus)> {
        let health_before = self
            .borrow_position_health(borrow_position, collateral_oracle, supply_oracle)
            .track_caller()?;
        if health_before.ltv < self.config.ltv_config().unhealthy_ltv {
            return Err(LendingError::PositionIsHealthy.into());
        }
        let liquidation = if health_before.ltv >= IFixedPoint::one() {
            let mut liquidation = LiquidationResultWithBonus {
                borrowed_atoms_to_repay: health_before.borrowed_atoms,
                collateral_atoms_to_liquidate: health_before.collateral_atoms,
                collateral_atoms_liquidation_bonus: 0,
            };
            liquidation.adjust_for_max_repay(max_repay_atoms);
            liquidation
        } else {
            compute_liquidation_with_fee(
                health_before.borrowed_atoms,
                self.supply_vault.mint_decimals(),
                supply_oracle,
                borrow_position.collateral_deposited_atoms(),
                self.collateral_vault.mint_decimals(),
                collateral_oracle,
                self.config.ltv_config().target_ltv_after_liquidation(),
                self.config.ltv_config().liquidation_bonus,
                max_repay_atoms,
            )
            .track_caller()?
        };
        Ok((health_before, liquidation))
    }

    pub(super) fn socialize_loss(
        &mut self,
        borrow_position: &mut BorrowPosition,
        collateral_oracle: &OracleRate,
        supply_oracle: &OracleRate,
    ) -> LendingResult<(u64, u64)> {
        let health = self
            .borrow_position_health(borrow_position, collateral_oracle, supply_oracle)
            .track_caller()?;
        if health.ltv < IFixedPoint::one() {
            return Err(LendingError::CannotSocializeDebtForHealthyPosition.into());
        }
        let debt = self
            .supply_vault
            .socialize_loss(borrow_position.borrowed_shares())?;
        let collateral_to_withdraw = borrow_position.collateral_deposited_atoms();
        borrow_position.repay_all();
        borrow_position.withdraw_collateral(collateral_to_withdraw)?;
        Ok((debt, collateral_to_withdraw))
    }
}

#[cfg(test)]
pub mod tests {

    use std::u64;

    use crate::{
        assert_eq_float,
        oracle::oracle_config::tests::{default_btc_oracle_rate, default_usd_oracle_rate},
        state::{
            collateral_vault::tests::{create_btc_collateral_vault, BTC},
            market_config::tests::test_config,
            supply_vault::tests::{create_usdc_supply_vault, USDC},
        },
    };

    use super::*;

    const INITIAL_USDC_DEPOSIT: u64 = USDC(10_000_000.);

    pub fn create_empty_btc_usdc_market() -> Market {
        Market {
            collateral_vault: create_btc_collateral_vault(),
            supply_vault: create_usdc_supply_vault(),
            config: test_config(),
        }
    }

    pub fn create_btc_usdc_market() -> Market {
        let mut market = create_empty_btc_usdc_market();
        let mut supply_position = SupplyPosition::default();
        market
            .lend(&mut supply_position, INITIAL_USDC_DEPOSIT)
            .unwrap();
        market
    }

    #[test]
    pub fn can_borrow() {
        let mut market = create_btc_usdc_market();
        let mut borrow_position = BorrowPosition::default();
        let collateral_oracle = default_btc_oracle_rate();
        let supply_oracle = default_usd_oracle_rate();
        market
            .deposit_collateral(&mut borrow_position, BTC(0.5))
            .unwrap();
        market
            .borrow(
                &mut borrow_position,
                USDC(100.),
                &supply_oracle,
                &collateral_oracle,
            )
            .unwrap();
        let health = market
            .borrow_position_health(&borrow_position, &collateral_oracle, &supply_oracle)
            .unwrap();
        assert_eq!(health.ltv, IFixedPoint::lit("0.002004004004004"));
        assert_eq!(health.borrow_value, IFixedPoint::lit("100.100000000000122"));
        assert_eq!(health.collateral_value, IFixedPoint::lit("49950"));
    }

    #[test]
    pub fn cant_borrow_more_than_max_ltv() {
        let mut market = create_btc_usdc_market();
        let mut borrow_position = BorrowPosition::default();
        let collateral_oracle = default_btc_oracle_rate();
        let supply_oracle = default_usd_oracle_rate();
        market
            .deposit_collateral(&mut borrow_position, BTC(0.5))
            .unwrap();
        assert_eq!(
            market
                .borrow(
                    &mut borrow_position,
                    USDC(100_000.),
                    &supply_oracle,
                    &collateral_oracle,
                )
                .err()
                .unwrap(),
            LendingError::MaxLtvReached
        );
    }

    #[test]
    pub fn cant_borrow_more_than_max_utilisation_rate() {
        let mut market = create_btc_usdc_market();
        let mut borrow_position = BorrowPosition::default();
        let collateral_oracle = default_btc_oracle_rate();
        let supply_oracle = default_usd_oracle_rate();
        market
            .deposit_collateral(&mut borrow_position, BTC(500.))
            .unwrap();
        assert_eq!(
            market
                .borrow(
                    &mut borrow_position,
                    INITIAL_USDC_DEPOSIT - INITIAL_USDC_DEPOSIT / 100,
                    &supply_oracle,
                    &collateral_oracle,
                )
                .err()
                .unwrap(),
            LendingError::MaxUtilisationRateReached
        );
    }

    #[test]
    pub fn cant_withdraw_more_collateral_than_owned() {
        let mut market = create_btc_usdc_market();
        let mut borrow_position = BorrowPosition::default();
        let collateral_oracle = default_btc_oracle_rate();
        let supply_oracle = default_usd_oracle_rate();
        market
            .deposit_collateral(&mut borrow_position, BTC(0.5))
            .unwrap();
        assert_eq!(
            market
                .withdraw_collateral(
                    &mut borrow_position,
                    BTC(0.5) + 1,
                    &supply_oracle,
                    &collateral_oracle,
                )
                .err()
                .unwrap(),
            LendingError::WithdrawalExceedsDeposited
        );
    }

    #[test]
    pub fn cant_withdraw_more_supply_than_owned() {
        let mut market = create_btc_usdc_market();
        let mut supply_position = SupplyPosition::default();
        market
            .lend(&mut supply_position, INITIAL_USDC_DEPOSIT)
            .unwrap();
        assert_eq!(
            market
                .withdraw(&mut supply_position, INITIAL_USDC_DEPOSIT + 1)
                .err()
                .unwrap(),
            LendingError::WithdrawalExceedsDeposited
        );
    }

    #[test]
    pub fn cant_liquidate_healthy_position() {
        let mut market = create_btc_usdc_market();
        let mut borrow_position = BorrowPosition::default();
        let collateral_oracle = default_btc_oracle_rate();
        let supply_oracle = default_usd_oracle_rate();
        market
            .deposit_collateral(&mut borrow_position, BTC(0.5))
            .unwrap();
        assert_eq!(
            market
                .liquidate(
                    &mut borrow_position,
                    &collateral_oracle,
                    &supply_oracle,
                    USDC(100.)
                )
                .err()
                .unwrap(),
            LendingError::PositionIsHealthy
        );
    }

    #[test]
    pub fn can_partially_liquidate_unhealthy_position() {
        let mut market = create_btc_usdc_market();
        let mut borrow_position = BorrowPosition::default();
        let collateral_oracle = default_btc_oracle_rate();
        let supply_oracle = default_usd_oracle_rate();
        market
            .deposit_collateral(&mut borrow_position, BTC(0.5))
            .unwrap();
        market
            .borrow(
                &mut borrow_position,
                USDC(20_000.),
                &supply_oracle,
                &collateral_oracle,
            )
            .unwrap();
        let supply_oracle =
            OracleRate::new(IFixedPoint::from_num(2.3), IFixedPoint::from_num(0.001));
        let ltv_before_liquidation = market
            .borrow_position_health(&borrow_position, &collateral_oracle, &supply_oracle)
            .unwrap()
            .ltv;
        let liquidation_result_with_ctx = market
            .liquidate(
                &mut borrow_position,
                &collateral_oracle,
                &supply_oracle,
                USDC(100.),
            )
            .unwrap();
        let ltv_after_liquidation = market
            .borrow_position_health(&borrow_position, &collateral_oracle, &supply_oracle)
            .unwrap()
            .ltv;
        assert_eq!(
            liquidation_result_with_ctx
                .liquidation_result_with_bonus
                .borrowed_atoms_to_repay,
            USDC(100.)
        );
        assert_eq_float!(
            liquidation_result_with_ctx
                .liquidation_result_with_bonus
                .collateral_atoms_liquidation_bonus as f64
                / liquidation_result_with_ctx
                    .liquidation_result_with_bonus
                    .collateral_atoms_to_liquidate as f64,
            market.config().ltv_config().liquidation_bonus.to_float(),
            0.0001
        );
        assert!(ltv_after_liquidation < ltv_before_liquidation);
        assert!(ltv_after_liquidation > market.config.ltv_config().unhealthy_ltv);
    }

    #[test]
    pub fn can_fully_liquidate_unhealthy_position() {
        let mut market = create_btc_usdc_market();
        let mut borrow_position = BorrowPosition::default();
        let collateral_oracle = default_btc_oracle_rate();
        let supply_oracle = default_usd_oracle_rate();
        market
            .deposit_collateral(&mut borrow_position, BTC(0.5))
            .unwrap();
        market
            .borrow(
                &mut borrow_position,
                USDC(20_000.),
                &supply_oracle,
                &collateral_oracle,
            )
            .unwrap();
        let supply_oracle =
            OracleRate::new(IFixedPoint::from_num(2.3), IFixedPoint::from_num(0.001));
        let position_snapshot = borrow_position.clone();
        let liquidation_result_with_ctx = market
            .liquidate(
                &mut borrow_position,
                &collateral_oracle,
                &supply_oracle,
                u64::MAX,
            )
            .unwrap();
        let health_after_liquidation = market
            .borrow_position_health(&borrow_position, &collateral_oracle, &supply_oracle)
            .unwrap();
        assert!(
            liquidation_result_with_ctx
                .liquidation_result_with_bonus
                .collateral_atoms_liquidation_bonus
                != 0
        );
        assert_eq_float!(
            liquidation_result_with_ctx
                .liquidation_result_with_bonus
                .collateral_atoms_liquidation_bonus as f64
                / liquidation_result_with_ctx
                    .liquidation_result_with_bonus
                    .collateral_atoms_to_liquidate as f64,
            market.config().ltv_config().liquidation_bonus.to_float(),
            0.0001
        );
        assert!(health_after_liquidation.ltv < market.config().ltv_config().unhealthy_ltv);
        assert_eq!(
            position_snapshot.collateral_deposited_atoms()
                - liquidation_result_with_ctx
                    .liquidation_result_with_bonus
                    .collateral_atoms_liquidation_bonus
                - liquidation_result_with_ctx
                    .liquidation_result_with_bonus
                    .collateral_atoms_to_liquidate,
            borrow_position.collateral_deposited_atoms()
        );
    }

    #[test]
    pub fn can_fully_liquidate_unhealthy_position_with_reduced_liquidation_fee() {
        let mut market = create_btc_usdc_market();
        let mut borrow_position = BorrowPosition::default();
        let collateral_oracle = default_btc_oracle_rate();
        let supply_oracle = default_usd_oracle_rate();
        market
            .deposit_collateral(&mut borrow_position, BTC(0.5))
            .unwrap();
        market
            .borrow(
                &mut borrow_position,
                USDC(20_000.),
                &supply_oracle,
                &collateral_oracle,
            )
            .unwrap();
        let supply_oracle =
            OracleRate::new(IFixedPoint::from_num(2.4), IFixedPoint::from_num(0.001));
        let position_snapshot = borrow_position.clone();
        let liquidation_result_with_ctx = market
            .liquidate(
                &mut borrow_position,
                &collateral_oracle,
                &supply_oracle,
                u64::MAX,
            )
            .unwrap();
        let health_after_liquidation = market
            .borrow_position_health(&borrow_position, &collateral_oracle, &supply_oracle)
            .unwrap();
        assert!(
            liquidation_result_with_ctx
                .liquidation_result_with_bonus
                .collateral_atoms_liquidation_bonus
                != 0
        );
        assert_eq_float!(
            liquidation_result_with_ctx
                .liquidation_result_with_bonus
                .collateral_atoms_liquidation_bonus as f64
                / liquidation_result_with_ctx
                    .liquidation_result_with_bonus
                    .collateral_atoms_to_liquidate as f64,
            0.04019,
            0.0001
        );
        assert!(health_after_liquidation.ltv < market.config().ltv_config().unhealthy_ltv);
        assert_eq!(
            position_snapshot.collateral_deposited_atoms()
                - liquidation_result_with_ctx
                    .liquidation_result_with_bonus
                    .collateral_atoms_liquidation_bonus
                - liquidation_result_with_ctx
                    .liquidation_result_with_bonus
                    .collateral_atoms_to_liquidate,
            borrow_position.collateral_deposited_atoms()
        );
    }

    #[test]
    pub fn can_fully_liquidate_unprofitable_position() {
        let mut market = create_btc_usdc_market();
        let mut borrow_position = BorrowPosition::default();
        let collateral_oracle = default_btc_oracle_rate();
        let supply_oracle = default_usd_oracle_rate();
        market
            .deposit_collateral(&mut borrow_position, BTC(0.5))
            .unwrap();
        market
            .borrow(
                &mut borrow_position,
                USDC(20_000.),
                &supply_oracle,
                &collateral_oracle,
            )
            .unwrap();
        let supply_oracle = OracleRate::new(IFixedPoint::from_num(3), IFixedPoint::from_num(0.001));
        let liquidation_result_with_ctx = market
            .liquidate(
                &mut borrow_position,
                &collateral_oracle,
                &supply_oracle,
                u64::MAX,
            )
            .unwrap();
        let health_after_liquidation = market
            .borrow_position_health(&borrow_position, &collateral_oracle, &supply_oracle)
            .unwrap();
        assert!(health_after_liquidation.ltv.is_zero());
        assert_eq!(
            liquidation_result_with_ctx
                .liquidation_result_with_bonus
                .collateral_atoms_liquidation_bonus,
            0
        );
        assert_eq!(borrow_position.collateral_deposited_atoms(), 0);
        assert!(borrow_position.borrowed_shares().is_zero());
    }

    #[test]
    pub fn cant_socialize_healthy_position() {
        let mut market = create_btc_usdc_market();
        let mut borrow_position = BorrowPosition::default();
        let collateral_oracle = default_btc_oracle_rate();
        let supply_oracle = default_usd_oracle_rate();
        market
            .deposit_collateral(&mut borrow_position, BTC(0.5))
            .unwrap();
        assert_eq!(
            market
                .socialize_loss(&mut borrow_position, &collateral_oracle, &supply_oracle)
                .err()
                .unwrap(),
            LendingError::CannotSocializeDebtForHealthyPosition
        );
    }

    #[test]
    pub fn can_socialize_unhealthy_position() {
        let mut market = create_btc_usdc_market();
        let mut borrow_position = BorrowPosition::default();
        let mut lending = SupplyPosition::default();
        market.lend(&mut lending, USDC(1_000_000.)).unwrap();

        let collateral_oracle = default_btc_oracle_rate();
        let supply_oracle = default_usd_oracle_rate();
        market
            .deposit_collateral(&mut borrow_position, BTC(0.5))
            .unwrap();
        market
            .borrow(
                &mut borrow_position,
                USDC(20_000.),
                &supply_oracle,
                &collateral_oracle,
            )
            .unwrap();
        let supply_oracle = OracleRate::new(IFixedPoint::from_num(3), IFixedPoint::from_num(0.001));
        let collateral_before_socialize = borrow_position.collateral_deposited_atoms();
        let collateral_withdrawn = market
            .socialize_loss(&mut borrow_position, &collateral_oracle, &supply_oracle)
            .unwrap();
        assert_eq!(collateral_before_socialize, collateral_withdrawn.1);
        assert_eq!(borrow_position.collateral_deposited_atoms(), 0);
        assert!(borrow_position.borrowed_shares().is_zero());

        let withdraw = market.withdraw_all(&mut lending).unwrap();
        assert!(withdraw < USDC(1_000_000.) - 1);
    }

    #[test]
    pub fn check_donate() {
        let mut market = create_empty_btc_usdc_market();
        let mut lending = SupplyPosition::default();
        market.lend(&mut lending, USDC(1_000_000.)).unwrap();
        let supply_before = market.supply_vault.total_supply().unwrap();
        market.donate_supply_atoms(USDC(100_000.)).unwrap();
        let supply_after = market.supply_vault.total_supply().unwrap();
        assert_eq!(supply_after - supply_before, USDC(100_000.) - 1); // rounding down error
        let withdraw = market.withdraw_all(&mut lending).unwrap();
        assert_eq!(withdraw, USDC(1_100_000.) - 1); // rounding down error
    }
}
