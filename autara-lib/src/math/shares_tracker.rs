use bytemuck::{Pod, Zeroable};

use crate::{
    error::{LendingError, LendingResult, LendingResultExt},
    interest_rate::interest_rate::InterestRate,
    math::rounding::RoundingMode,
};

use super::{safe_math::SafeMath, ufixed_point::UFixedPoint};

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Pod, Zeroable)]
#[cfg_attr(
    feature = "client",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct SharesTracker {
    total_shares: UFixedPoint,
    atoms_per_share: UFixedPoint,
}

impl SharesTracker {
    pub fn new() -> Self {
        SharesTracker {
            total_shares: UFixedPoint::from_u64(0),
            atoms_per_share: UFixedPoint::from_u64(1),
        }
    }

    pub fn total_shares(&self) -> UFixedPoint {
        self.total_shares
    }

    pub fn atoms_per_share(&self) -> UFixedPoint {
        self.atoms_per_share
    }

    pub fn initialize(&mut self) {
        self.total_shares = UFixedPoint::from_u64(0);
        self.atoms_per_share = UFixedPoint::from_u64(1);
    }

    pub fn atoms_to_shares(&self, atoms: u64) -> LendingResult<UFixedPoint> {
        UFixedPoint::from_u64(atoms).safe_div(self.atoms_per_share)
    }

    pub fn shares_to_atoms(
        &self,
        shares: UFixedPoint,
        rounding: RoundingMode,
    ) -> LendingResult<u64> {
        shares
            .safe_mul(self.atoms_per_share)
            .and_then(|x| x.as_u64_rounded(rounding))
    }

    pub fn total_atoms(&self, rounding: RoundingMode) -> LendingResult<u64> {
        self.total_shares
            .safe_mul(self.atoms_per_share)
            .and_then(|x| x.as_u64_rounded(rounding))
    }

    pub fn deposit_atoms(&mut self, atoms: u64) -> LendingResult<UFixedPoint> {
        let shares = self.atoms_to_shares(atoms)?;
        self.total_shares = self.total_shares.safe_add(shares)?;
        Ok(shares)
    }

    pub fn withdraw_atoms(&mut self, atoms: u64) -> LendingResult<UFixedPoint> {
        let shares = self.atoms_to_shares(atoms)?;
        self.total_shares = self
            .total_shares
            .safe_sub(shares)
            .map_err(|_| LendingError::SharesOverflow)?;
        Ok(shares)
    }

    pub fn withdraw_atoms_capped(
        &mut self,
        atoms: u64,
        max_shares: UFixedPoint,
        rounding: RoundingMode,
    ) -> LendingResult<(u64, UFixedPoint)> {
        let shares = self.atoms_to_shares(atoms)?;
        if shares > max_shares {
            let atoms = self.shares_to_atoms(self.total_shares, rounding)?;
            self.total_shares = self
                .total_shares
                .safe_sub(max_shares)
                .map_err(|_| LendingError::SharesOverflow)?;
            return Ok((atoms, max_shares));
        }
        self.total_shares = self
            .total_shares
            .safe_sub(shares)
            .map_err(|_| LendingError::SharesOverflow)?;
        Ok((atoms, shares))
    }

    pub fn withdraw_shares(
        &mut self,
        shares: UFixedPoint,
        rounding: RoundingMode,
    ) -> LendingResult<u64> {
        let atoms = self.shares_to_atoms(shares, rounding)?;
        self.total_shares = self
            .total_shares
            .safe_sub(shares)
            .map_err(|_| LendingError::SharesOverflow)?;
        Ok(atoms)
    }

    pub fn apply_interest_rate(&mut self, interest_rate: InterestRate) -> LendingResult<()> {
        if self.total_shares.is_zero() {
            return Ok(());
        }
        let interest = interest_rate.interest(self.atoms_per_share)?;
        self.atoms_per_share = interest.safe_add(self.atoms_per_share)?.try_into()?;
        Ok(())
    }

    pub fn apply_interest_rate_with_fee(
        &mut self,
        interest_rate: InterestRate,
        fee: UFixedPoint,
    ) -> LendingResult<UFixedPoint> {
        let total_atoms_before = self.total_atoms(RoundingMode::RoundDown)?;
        if total_atoms_before == 0 {
            return Ok(UFixedPoint::from_u64(0));
        }
        if interest_rate.rate().is_negative() {
            self.apply_interest_rate(interest_rate)?;
            return Err(LendingError::NegativeInterestRate.into());
        }
        let interest_atoms = interest_rate.interest(total_atoms_before)?;
        let fee_atoms = interest_atoms.safe_mul(fee)?;
        let net_interest_atoms = interest_atoms.safe_sub(fee_atoms)?;
        let net_interest_rate = net_interest_atoms.safe_div(total_atoms_before)?;
        let net_interest_atoms_per_share = net_interest_rate.safe_mul(self.atoms_per_share)?;
        self.atoms_per_share = net_interest_atoms_per_share
            .safe_add(self.atoms_per_share)?
            .try_into()?;
        let fee_shares: UFixedPoint = fee_atoms.safe_div(self.atoms_per_share)?.try_into()?;
        self.total_shares = fee_shares.safe_add(self.total_shares)?;
        Ok(fee_shares)
    }

    pub fn donate_atoms(&mut self, atoms: u64) -> LendingResult<()> {
        if self.total_shares.is_zero() {
            return Err(LendingError::CantModifySharePriceIfZeroShares.into()).with_msg("donate");
        }
        let additional_atoms_per_share =
            UFixedPoint::from_u64(atoms).safe_div(self.total_shares)?;
        self.atoms_per_share = self.atoms_per_share.safe_add(additional_atoms_per_share)?;
        Ok(())
    }

    pub fn socialize_loss_atoms(&mut self, atoms: u64) -> LendingResult<()> {
        if self.total_shares.is_zero() {
            return Err(LendingError::CantModifySharePriceIfZeroShares.into())
                .with_msg("socialize_loss");
        }
        let atoms_per_share = UFixedPoint::from_u64(atoms).safe_div(self.total_shares)?;
        self.atoms_per_share = self.atoms_per_share.safe_sub(atoms_per_share)?;
        Ok(())
    }
}

impl Default for SharesTracker {
    fn default() -> Self {
        SharesTracker::new()
    }
}

#[cfg(test)]
pub mod tests {
    use crate::math::{bps::bps_to_fixed_point, ifixed_point::IFixedPoint};

    use super::*;

    #[test]
    pub fn deposit_withdraw() {
        let mut tracker = SharesTracker::new();
        let atoms = 100000;
        let shares = tracker.deposit_atoms(atoms).unwrap();
        assert_eq!(shares, UFixedPoint::from_u64(atoms));
        assert_eq!(tracker.total_atoms(RoundingMode::RoundDown).unwrap(), atoms);
        let withdrawn_atoms = tracker
            .withdraw_shares(shares, RoundingMode::RoundDown)
            .unwrap();
        assert_eq!(withdrawn_atoms, atoms);
        assert_eq!(tracker.total_atoms(RoundingMode::RoundDown).unwrap(), 0);
    }

    #[test]
    pub fn apply_negative_interest_rate() {
        let mut tracker = SharesTracker::new();
        let initial_atoms = 1000000;
        let shares = tracker.deposit_atoms(initial_atoms).unwrap();
        let interest_rate = InterestRate::new(IFixedPoint::lit("-0.1")); // -10% interest
        tracker.apply_interest_rate(interest_rate).unwrap();
        let total_supply = tracker.total_atoms(RoundingMode::RoundDown).unwrap();
        assert_eq!(total_supply, 899999);
        let shares_minted = tracker
            .withdraw_shares(shares, RoundingMode::RoundDown)
            .unwrap();
        assert_eq!(shares_minted, 899999);
    }

    #[test]
    pub fn apply_fee_on_interest_rate() {
        let mut tracker = SharesTracker::new();
        let initial_atoms = 1000000000;
        tracker.deposit_atoms(initial_atoms).unwrap();
        let interest_rate = InterestRate::new(IFixedPoint::lit("0.5")); // 50% interest
        let shares_minted = tracker
            .apply_interest_rate_with_fee(interest_rate, bps_to_fixed_point(1_000)) // 10% fee
            .unwrap();
        let total_supply = tracker.total_atoms(RoundingMode::RoundDown).unwrap();
        assert_eq!(total_supply, 1499999999);
        let withdraw_fees = tracker
            .withdraw_shares(shares_minted, RoundingMode::RoundDown)
            .unwrap();
        assert_eq!(withdraw_fees, (total_supply - initial_atoms) / 10);
    }

    #[test]
    pub fn deposit_withdraw_zero_atoms() {
        let mut tracker = SharesTracker::new();
        let atoms = 0;
        let shares = tracker.deposit_atoms(atoms).unwrap();
        assert_eq!(shares, UFixedPoint::from_u64(0));
        assert_eq!(tracker.total_atoms(RoundingMode::RoundDown).unwrap(), 0);
        let withdrawn_atoms = tracker
            .withdraw_shares(shares, RoundingMode::RoundDown)
            .unwrap();
        assert_eq!(withdrawn_atoms, 0);
        assert_eq!(tracker.total_atoms(RoundingMode::RoundDown).unwrap(), 0);
    }

    #[test]
    pub fn withdraw_more_than_available() {
        let mut tracker = SharesTracker::new();
        let atoms = 1000;
        let shares = tracker.deposit_atoms(atoms).unwrap();
        let result = tracker.withdraw_shares(
            shares.safe_add(UFixedPoint::from_u64(1)).unwrap(),
            RoundingMode::RoundDown,
        );
        assert_eq!(result.unwrap_err(), LendingError::SharesOverflow);
        let result = tracker.withdraw_atoms(atoms + 1);
        assert_eq!(result.unwrap_err(), LendingError::SharesOverflow);
    }

    #[test]
    pub fn apply_interest_with_zero_supply() {
        let mut tracker = SharesTracker::new();
        let interest_rate = InterestRate::new(IFixedPoint::from_u64(1));
        let result = tracker.apply_interest_rate(interest_rate);
        assert!(result.is_ok());
        assert_eq!(tracker.total_atoms(RoundingMode::RoundDown).unwrap(), 0);
    }

    #[test]
    pub fn apply_interest_with_fee_greater_than_interest() {
        let mut tracker = SharesTracker::new();
        let initial_atoms = 1000;
        tracker.deposit_atoms(initial_atoms).unwrap();
        let interest_rate = InterestRate::new(IFixedPoint::from_u64(1));
        let excessive_fee = UFixedPoint::from_u64(2);
        let result = tracker.apply_interest_rate_with_fee(interest_rate, excessive_fee);
        assert!(result.is_err());
    }

    #[test]
    pub fn test_donate_and_socialize_loss() {
        let mut tracker = SharesTracker::new();
        let initial_atoms = 1000;
        tracker.deposit_atoms(initial_atoms).unwrap();
        tracker.donate_atoms(500).unwrap();
        let total_after_donation = tracker.total_atoms(RoundingMode::RoundDown).unwrap();
        assert_eq!(total_after_donation, 1500);
        tracker.socialize_loss_atoms(300).unwrap();
        let total_after_loss = tracker.total_atoms(RoundingMode::RoundDown).unwrap();
        assert_eq!(total_after_loss, 1200);
    }

    #[test]
    pub fn test_deposit_withdraw_consitency_with_magnitude() {
        let small_deposit = 10;
        let big_deposit = 1_000_000_000_000;
        for [small_deposit_first, small_withdraw_first] in
            [[true, false], [false, true], [true, true], [false, false]]
        {
            let mut tracker = SharesTracker::new();
            let (small_shares, big_shares) = if small_deposit_first {
                (
                    tracker.deposit_atoms(small_deposit).unwrap(),
                    tracker.deposit_atoms(big_deposit).unwrap(),
                )
            } else {
                let big_shares = tracker.deposit_atoms(big_deposit).unwrap();
                let small_shares = tracker.deposit_atoms(small_deposit).unwrap();
                (small_shares, big_shares)
            };
            let (small_withdrawn, big_withdrawn) = if small_withdraw_first {
                (
                    tracker
                        .withdraw_shares(small_shares, RoundingMode::RoundDown)
                        .unwrap(),
                    tracker
                        .withdraw_shares(big_shares, RoundingMode::RoundDown)
                        .unwrap(),
                )
            } else {
                let big_withdrawn = tracker
                    .withdraw_shares(big_shares, RoundingMode::RoundDown)
                    .unwrap();
                let small_withdrawn = tracker
                    .withdraw_shares(small_shares, RoundingMode::RoundDown)
                    .unwrap();
                (small_withdrawn, big_withdrawn)
            };
            assert_eq!(small_withdrawn, small_deposit);
            assert_eq!(big_withdrawn, big_deposit);
        }
    }
}
