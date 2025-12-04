use arch_program::pubkey::Pubkey;
use borsh::{BorshDeserialize, BorshSerialize};
use bytemuck::{Pod, Zeroable};

use crate::{
    constant::MAX_EXPONENT_ABS,
    error::{LendingError, LendingResult, LendingResultExt},
    interest_rate::{
        interest_rate_kind::InterestRateCurveKind,
        interest_rate_per_second::InterestRatePerSecond,
        lending_interest_rate::{LendingInterestRateCurveMut, MarketBorrowRateParameters},
        pod_interest_rate::PodInterestRateCurve,
    },
    math::{
        bps::bps_from_fixed_point, ifixed_point::IFixedPoint, rounding::RoundingMode,
        safe_math::SafeMath, shares_tracker::SharesTracker, ufixed_point::UFixedPoint,
    },
    oracle::{oracle_config::OracleConfig, pod_oracle_provider::PodOracleProvider},
    padding::Padding,
};

crate::validate_struct!(SupplyVault, 720);

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[cfg_attr(
    feature = "client",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct SupplyVault {
    /// The mint of the supply token
    mint: Pubkey,
    /// The decimals of the supply token
    mint_decimals: u64,
    /// The token account that holds the supply tokens
    vault: Pubkey,
    /// The oracle config to use for this supply vault
    oracle_config: OracleConfig,
    /// Track the lending shares and total atoms supplied
    supply_shares_tracker: SharesTracker,
    /// Track the borrow shares and total atoms borrowed
    borrow_shares_tracker: SharesTracker,
    /// The interest rate curve to use for this supply vault
    interest_rate_curve: PodInterestRateCurve,
    /// The last borrow interest rate per second
    last_borrow_interest_rate: InterestRatePerSecond,
    /// The last time the supply vault was updated (in seconds)
    last_update_unix_timestamp: i64,
    /// The pending protocol fee shares to be redeemed
    pending_protocol_fee_shares: UFixedPoint,
    /// The pending curator fee shares to be redeemed
    pending_curator_fee_shares: UFixedPoint,
    pad: Padding<192>,
}

impl SupplyVault {
    pub fn initialize(
        &mut self,
        mint: Pubkey,
        mint_decimals: u64,
        vault: Pubkey,
        oracle_config: OracleConfig,
        interest_rate: InterestRateCurveKind,
        timestamp: i64,
    ) -> LendingResult {
        if mint_decimals as i64 > MAX_EXPONENT_ABS {
            return Err(LendingError::UnsupportedMintDecimals.into()).with_msg("supply vault");
        }
        if !interest_rate.is_valid() {
            return Err(LendingError::InvalidCurve.into());
        }
        oracle_config.validate()?;
        self.mint = mint;
        self.mint_decimals = mint_decimals;
        self.vault = vault;
        self.oracle_config = oracle_config;
        self.interest_rate_curve = PodInterestRateCurve::from(interest_rate);
        self.supply_shares_tracker.initialize();
        self.borrow_shares_tracker.initialize();
        self.last_update_unix_timestamp = timestamp;
        Ok(())
    }

    pub fn vault(&self) -> &Pubkey {
        &self.vault
    }

    pub fn mint(&self) -> &Pubkey {
        &self.mint
    }

    pub fn mint_decimals(&self) -> u8 {
        self.mint_decimals as u8
    }

    pub fn supply_shares_tracker(&self) -> &SharesTracker {
        &self.supply_shares_tracker
    }

    pub fn borrow_shares_tracker(&self) -> &SharesTracker {
        &self.borrow_shares_tracker
    }

    pub fn utilisation_rate(&self) -> LendingResult<IFixedPoint> {
        let total_supply = self.total_supply()?;
        let total_borrowed = self.total_borrow()?;
        Self::compute_utilisation_rate(total_borrowed, total_supply)
    }

    pub fn interest_rate_curve(&self) -> &PodInterestRateCurve {
        &self.interest_rate_curve
    }

    pub fn total_supply(&self) -> LendingResult<u64> {
        self.supply_shares_tracker
            .total_atoms(RoundingMode::RoundDown)
    }

    pub fn total_borrow(&self) -> LendingResult<u64> {
        self.borrow_shares_tracker
            .total_atoms(RoundingMode::RoundUp)
    }

    pub fn oracle_provider(&self) -> &PodOracleProvider {
        self.oracle_config.oracle_provider()
    }

    pub fn oracle_config(&self) -> &OracleConfig {
        &self.oracle_config
    }

    pub fn borrow_shares_to_atoms(&self, shares: UFixedPoint) -> LendingResult<u64> {
        self.borrow_shares_tracker
            .shares_to_atoms(shares, RoundingMode::RoundUp)
    }

    pub fn set_oracle_config(&mut self, oracle_config: OracleConfig) {
        self.oracle_config = oracle_config;
    }

    pub fn last_borrow_interest_rate(&self) -> InterestRatePerSecond {
        self.last_borrow_interest_rate
    }

    pub fn get_summary(&self) -> LendingResult<SupplyVaultSummary> {
        let total_supply = self.total_supply()?;
        let total_borrow = self.total_borrow()?;
        let utilisation_rate = Self::compute_utilisation_rate(total_borrow, total_supply)?;
        Ok(SupplyVaultSummary {
            last_update_unix_timestamp: self.last_update_unix_timestamp,
            total_supply,
            total_borrow,
            utilisation_rate,
            pending_curator_fee_atoms: self
                .supply_shares_tracker
                .shares_to_atoms(self.pending_curator_fee_shares, RoundingMode::RoundDown)?,
            pending_protocol_fee_atoms: self
                .supply_shares_tracker
                .shares_to_atoms(self.pending_protocol_fee_shares, RoundingMode::RoundDown)?,
            borrow_interest_rate: self.last_borrow_interest_rate,
            lending_interest_rate: self
                .last_borrow_interest_rate
                .adjust_for_utilisation_rate(utilisation_rate)?,
        })
    }

    fn compute_utilisation_rate(
        total_borrowed: u64,
        total_supply: u64,
    ) -> LendingResult<IFixedPoint> {
        IFixedPoint::from_ratio(total_borrowed, total_supply).or_else(|err| {
            if total_borrowed != 0 {
                Err(err)
            } else {
                Ok(IFixedPoint::zero())
            }
        })
    }
}

impl SupplyVault {
    pub(super) fn sync_clock(
        &mut self,
        unix_timestamp: i64,
        lending_market_fee: UFixedPoint,
        fee_percent_for_protocol_in_bps: u16,
    ) -> LendingResult {
        if unix_timestamp > self.last_update_unix_timestamp {
            let elapsed = (unix_timestamp - self.last_update_unix_timestamp) as u64;
            let utilisation_rate = self.utilisation_rate().track_caller()?;
            let params = MarketBorrowRateParameters {
                utilisation_rate: &utilisation_rate,
                elapsed_seconds_since_last_update: elapsed,
            };
            let borrow_interest_rate = self
                .interest_rate_curve
                .interest_rate_kind_mut()
                .borrow_rate_per_second(params)
                .track_caller()?;
            let borrow_rate_during_elapsed = borrow_interest_rate
                .coumpounding_interest_rate_during_elapsed_seconds(elapsed)
                .track_caller()?;
            self.borrow_shares_tracker
                .apply_interest_rate(borrow_rate_during_elapsed)
                .track_caller()?;
            let lending_interest_rate_during_elapsed = borrow_rate_during_elapsed
                .adjust_for_utilisation_rate(utilisation_rate)
                .track_caller()?;
            let fee_shares = self
                .supply_shares_tracker
                .apply_interest_rate_with_fee(
                    lending_interest_rate_during_elapsed,
                    lending_market_fee,
                )
                .track_caller()?;
            let protocol_fee_shares =
                bps_from_fixed_point(fee_percent_for_protocol_in_bps as u64, fee_shares)?;
            let curator_fee_shares = fee_shares.safe_sub(protocol_fee_shares)?;
            self.pending_protocol_fee_shares = self
                .pending_protocol_fee_shares
                .safe_add(protocol_fee_shares)?;
            self.pending_curator_fee_shares = self
                .pending_curator_fee_shares
                .safe_add(curator_fee_shares)?;
            self.last_borrow_interest_rate = borrow_interest_rate;
            self.last_update_unix_timestamp = unix_timestamp;
        }
        Ok(())
    }

    pub(super) fn lend(&mut self, atoms: u64) -> LendingResult<UFixedPoint> {
        self.supply_shares_tracker.deposit_atoms(atoms)
    }

    pub(super) fn withdraw_shares(&mut self, shares: UFixedPoint) -> LendingResult<u64> {
        self.supply_shares_tracker
            .withdraw_shares(shares, RoundingMode::RoundDown)
    }

    pub(super) fn withdraw_atoms(&mut self, atoms: u64) -> LendingResult<UFixedPoint> {
        self.supply_shares_tracker.withdraw_atoms(atoms)
    }

    pub(super) fn borrow(&mut self, atoms: u64) -> LendingResult<UFixedPoint> {
        self.borrow_shares_tracker.deposit_atoms(atoms)
    }

    pub(super) fn repay_shares(&mut self, shares: UFixedPoint) -> LendingResult<u64> {
        self.borrow_shares_tracker
            .withdraw_shares(shares, RoundingMode::RoundUp)
    }

    pub(super) fn repay_atoms(&mut self, atoms: u64) -> LendingResult<UFixedPoint> {
        self.borrow_shares_tracker.withdraw_atoms(atoms)
    }

    pub(super) fn repay_atoms_capped(
        &mut self,
        atoms: u64,
        max_shares: UFixedPoint,
    ) -> LendingResult<(u64, UFixedPoint)> {
        self.borrow_shares_tracker
            .withdraw_atoms_capped(atoms, max_shares, RoundingMode::RoundUp)
    }

    pub(super) fn redeem_protocol_fees(&mut self) -> LendingResult<u64> {
        let shares = self.pending_protocol_fee_shares;
        self.pending_protocol_fee_shares = UFixedPoint::zero();
        self.withdraw_shares(shares)
    }

    pub(super) fn redeem_curator_fees(&mut self) -> LendingResult<u64> {
        let shares = self.pending_curator_fee_shares;
        self.pending_curator_fee_shares = UFixedPoint::zero();
        self.withdraw_shares(shares)
    }

    pub(super) fn socialize_loss(&mut self, debt_shares: UFixedPoint) -> LendingResult<u64> {
        let debt = self
            .borrow_shares_tracker
            .withdraw_shares(debt_shares, RoundingMode::RoundUp)?;
        self.supply_shares_tracker.socialize_loss_atoms(debt)?;
        Ok(debt)
    }

    pub(super) fn donate_supply(&mut self, atoms: u64) -> LendingResult {
        self.supply_shares_tracker.donate_atoms(atoms)?;
        Ok(())
    }
}

#[repr(C)]
#[derive(
    Debug, Clone, Copy, Pod, Zeroable, PartialEq, Eq, BorshSerialize, BorshDeserialize, Default,
)]
#[cfg_attr(
    feature = "client",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct SupplyVaultSummary {
    pub last_update_unix_timestamp: i64,
    pub total_supply: u64,
    pub total_borrow: u64,
    pub pending_curator_fee_atoms: u64,
    pub pending_protocol_fee_atoms: u64,
    pub utilisation_rate: IFixedPoint,
    pub borrow_interest_rate: InterestRatePerSecond,
    pub lending_interest_rate: InterestRatePerSecond,
}

#[cfg(test)]
pub mod tests {
    use arch_program::pubkey::Pubkey;

    use crate::{
        constant::SECONDS_PER_YEAR,
        interest_rate::interest_rate_kind::InterestRateCurveKind,
        math::bps::{bps_to_fixed_point, percent_to_bps},
        oracle::oracle_config::tests::usd_oracle_config,
    };

    use super::*;

    #[allow(non_snake_case)]
    pub const fn USDC(amount: f64) -> u64 {
        (amount * 1_000_000.0) as u64
    }

    pub const USDC_DECIMALS: u64 = 6;

    pub fn create_usdc_supply_vault() -> SupplyVault {
        SupplyVault {
            mint: Pubkey::new_unique(),
            mint_decimals: USDC_DECIMALS,
            vault: Pubkey::new_unique(),
            oracle_config: usd_oracle_config(),
            supply_shares_tracker: Default::default(),
            borrow_shares_tracker: Default::default(),
            last_borrow_interest_rate: InterestRatePerSecond::approximate_from_apy(10.),
            interest_rate_curve: InterestRateCurveKind::new_approximate_fixed_apy(0.1).into(),
            last_update_unix_timestamp: 0,
            pending_protocol_fee_shares: UFixedPoint::zero(),
            pending_curator_fee_shares: UFixedPoint::zero(),
            pad: Padding::default(),
        }
    }

    #[test]
    pub fn check_lend_withdraw() {
        let mut vault = create_usdc_supply_vault();
        let deposit = 100000000;
        let shares = vault.lend(deposit).unwrap();
        let total_supply = vault.total_supply().unwrap();
        assert_eq!(total_supply, deposit);
        let withdraw = vault.withdraw_shares(shares).unwrap();
        assert_eq!(withdraw, deposit);
    }

    #[test]
    pub fn check_borrow_repay() {
        let mut vault = create_usdc_supply_vault();
        let borrow = 1000000;
        let shares = vault.borrow(borrow).unwrap();
        let total_borrow = vault.total_borrow().unwrap();
        assert_eq!(total_borrow, borrow);
        let repay = vault.repay_shares(shares).unwrap();
        assert_eq!(repay, borrow);
    }

    #[test]
    pub fn check_utilisation_rate() {
        let mut vault = create_usdc_supply_vault();
        vault.lend(100000000).unwrap();
        vault.borrow(1000000).unwrap();
        let utilisation_rate = vault.utilisation_rate().unwrap();
        assert_eq!(utilisation_rate, IFixedPoint::lit("0.009999999999998"));
    }

    #[test]
    pub fn check_update_without_fee() {
        let mut vault = create_usdc_supply_vault();
        let deposit = 100000000;
        let borrow = 1000000;
        vault.lend(deposit).unwrap();
        vault.borrow(borrow).unwrap();
        let total_borrow_before_update = vault.total_borrow().unwrap();
        let total_supply_before_update = vault.total_supply().unwrap();
        let now = SECONDS_PER_YEAR as i64;
        vault.sync_clock(now, UFixedPoint::zero(), 0).unwrap();
        assert_eq!(vault.last_update_unix_timestamp, now);
        let total_borrow_after_update = vault.total_borrow().unwrap();
        let total_supply_after_update = vault.total_supply().unwrap();
        let total_borrow_interest = total_borrow_after_update - total_borrow_before_update;
        let total_supply_interest = total_supply_after_update - total_supply_before_update;
        assert_eq!(total_borrow_interest, total_supply_interest + 1); // rounding
        assert_eq!(total_borrow_interest, (borrow as f64 * 0.1) as u64);
    }

    #[test]
    pub fn check_update_with_fee() {
        let mut vault = create_usdc_supply_vault();
        let deposit = 100000000;
        let borrow = 1000000;
        vault.lend(deposit).unwrap();
        vault.borrow(borrow).unwrap();
        let total_borrow_before_update = vault.total_borrow().unwrap();
        let total_supply_before_update = vault.total_supply().unwrap();
        let now = SECONDS_PER_YEAR as i64;
        vault
            .sync_clock(
                now,
                bps_to_fixed_point(percent_to_bps(10)),
                percent_to_bps(50) as u16,
            )
            .unwrap();
        assert_eq!(vault.last_update_unix_timestamp, now);
        let total_borrow_after_update = vault.total_borrow().unwrap();
        let total_supply_after_update = vault.total_supply().unwrap();
        let total_borrow_interest = total_borrow_after_update - total_borrow_before_update;
        let total_supply_interest = total_supply_after_update - total_supply_before_update;
        assert_eq!(total_borrow_interest, total_supply_interest + 1); // rounding
        assert_eq!(total_borrow_interest, (borrow as f64 * 0.1) as u64);
        let protocol_fee_earned = vault
            .supply_shares_tracker
            .shares_to_atoms(vault.pending_protocol_fee_shares, RoundingMode::RoundDown)
            .unwrap();
        let curator_fee_earned = vault
            .supply_shares_tracker
            .shares_to_atoms(vault.pending_curator_fee_shares, RoundingMode::RoundDown)
            .unwrap();
        assert_eq!(protocol_fee_earned, curator_fee_earned);
        assert_eq!(
            protocol_fee_earned + curator_fee_earned,
            total_supply_interest / 10 - 1
        );
    }

    #[test]
    pub fn check_donate_and_socialize() {
        let mut vault = create_usdc_supply_vault();
        let deposit = 100000000;
        let borrow = 1000000;
        vault.lend(deposit).unwrap();
        vault.borrow(borrow).unwrap();
        let total_borrow_before_donate = vault.total_borrow().unwrap();
        let total_supply_before_donate = vault.total_supply().unwrap();
        vault.donate_supply(10000000).unwrap();
        let total_borrow_after_donate = vault.total_borrow().unwrap();
        let total_supply_after_donate = vault.total_supply().unwrap();
        assert_eq!(total_borrow_before_donate, total_borrow_after_donate);
        assert_eq!(
            total_supply_after_donate,
            total_supply_before_donate + 10000000 - 1 // -1 because of rounding
        );
        let debt_shares = vault.borrow_shares_tracker().total_shares();
        vault.socialize_loss(debt_shares).unwrap();
        let total_borrow_after_socialize = vault.total_borrow().unwrap();
        let total_supply_after_socialize = vault.total_supply().unwrap();
        assert_eq!(total_borrow_after_socialize, 0);
        assert_eq!(
            total_supply_after_socialize,
            total_supply_after_donate - total_borrow_after_donate
        );
    }
}
