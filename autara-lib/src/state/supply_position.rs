use arch_program::pubkey::Pubkey;
use bytemuck::{Pod, Zeroable};

use crate::{
    error::LendingError,
    math::{safe_math::SafeMath, ufixed_point::UFixedPoint},
    padding::Padding,
};

use super::super::error::LendingResult;

crate::validate_struct!(SupplyPosition, 216);

#[repr(C)]
#[derive(Default, Pod, Zeroable, Debug, Clone, Copy)]
#[cfg_attr(
    feature = "client",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct SupplyPosition {
    /// The authority which can manage this lending position
    authority: Pubkey,
    /// The market this lending position is associated with
    market: Pubkey,
    /// Track the total deposited atoms, shares withdrawn will decrease this value proportionally
    deposited_atoms: u64,
    /// Track the total lending shares of supply vault owned by this position
    shares: UFixedPoint,
    pad: Padding<128>,
}

impl SupplyPosition {
    pub fn new(authority: Pubkey, market: Pubkey) -> Self {
        Self {
            authority,
            market,
            deposited_atoms: 0,
            shares: UFixedPoint::zero(),
            pad: Padding::default(),
        }
    }

    pub fn initialize(&mut self, authority: Pubkey, market: Pubkey) {
        self.authority = authority;
        self.market = market;
        self.deposited_atoms = 0;
        self.shares = UFixedPoint::zero();
    }

    #[inline(always)]
    pub fn authority(&self) -> &Pubkey {
        &self.authority
    }

    #[inline(always)]
    pub fn market(&self) -> &Pubkey {
        &self.market
    }

    #[inline(always)]
    pub fn deposited_atoms(&self) -> u64 {
        self.deposited_atoms
    }

    #[inline(always)]
    pub fn shares(&self) -> UFixedPoint {
        self.shares
    }

    pub fn lend(&mut self, atoms: u64, shares: UFixedPoint) -> LendingResult {
        self.deposited_atoms = self.deposited_atoms.safe_add(atoms)?;
        self.shares = self.shares.safe_add(shares)?;
        Ok(())
    }

    pub fn withdraw(&mut self, shares: UFixedPoint) -> LendingResult {
        let adjusted_atoms = match shares.cmp(&self.shares) {
            std::cmp::Ordering::Less => shares
                .safe_div(self.shares)?
                .safe_mul(self.deposited_atoms)?
                .as_u64_rounded_down()?,
            std::cmp::Ordering::Equal => self.deposited_atoms,
            std::cmp::Ordering::Greater => {
                return Err(LendingError::WithdrawalExceedsDeposited.into())
            }
        };
        self.deposited_atoms = self.deposited_atoms.safe_sub(adjusted_atoms)?;
        self.shares = self.shares.safe_sub(shares)?;
        Ok(())
    }

    pub fn withdraw_all(&mut self) {
        self.deposited_atoms = 0;
        self.shares = UFixedPoint::zero();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_position() -> SupplyPosition {
        SupplyPosition::new(Pubkey::new_unique(), Pubkey::new_unique())
    }

    #[test]
    fn lend_withdraw_roundtrip() {
        let mut pos = create_position();
        let atoms = 1_000_000u64;
        let shares = UFixedPoint::from_u64(1_000_000);
        pos.lend(atoms, shares).unwrap();
        assert_eq!(pos.deposited_atoms(), atoms);
        assert_eq!(pos.shares(), shares);
        pos.withdraw(shares).unwrap();
        assert_eq!(pos.deposited_atoms(), 0);
        assert!(pos.shares().is_zero());
    }

    #[test]
    fn cannot_withdraw_more_than_shares() {
        let mut pos = create_position();
        pos.lend(1000, UFixedPoint::from_u64(1000)).unwrap();
        let result = pos.withdraw(UFixedPoint::from_u64(1001));
        assert!(result.is_err());
    }

    #[test]
    fn partial_withdraw_reduces_proportionally() {
        let mut pos = create_position();
        pos.lend(1000, UFixedPoint::from_u64(1000)).unwrap();
        // Withdraw half the shares
        pos.withdraw(UFixedPoint::from_u64(500)).unwrap();
        assert_eq!(pos.deposited_atoms(), 500);
        assert_eq!(pos.shares(), UFixedPoint::from_u64(500));
    }

    #[test]
    fn withdraw_all_clears_position() {
        let mut pos = create_position();
        pos.lend(1000, UFixedPoint::from_u64(1000)).unwrap();
        pos.withdraw_all();
        assert_eq!(pos.deposited_atoms(), 0);
        assert!(pos.shares().is_zero());
    }

    #[test]
    fn multiple_lends_accumulate() {
        let mut pos = create_position();
        pos.lend(100, UFixedPoint::from_u64(100)).unwrap();
        pos.lend(200, UFixedPoint::from_u64(200)).unwrap();
        pos.lend(300, UFixedPoint::from_u64(300)).unwrap();
        assert_eq!(pos.deposited_atoms(), 600);
        assert_eq!(pos.shares(), UFixedPoint::from_u64(600));
    }

    #[test]
    fn initialize_resets_position() {
        let mut pos = create_position();
        pos.lend(1000, UFixedPoint::from_u64(1000)).unwrap();
        let new_auth = Pubkey::new_unique();
        let new_market = Pubkey::new_unique();
        pos.initialize(new_auth, new_market);
        assert_eq!(pos.deposited_atoms(), 0);
        assert!(pos.shares().is_zero());
        assert_eq!(pos.authority(), &new_auth);
        assert_eq!(pos.market(), &new_market);
    }
}
