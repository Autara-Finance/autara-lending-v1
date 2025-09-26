use arch_program::account::{next_account_info, AccountInfo};
use autara_lib::state::global_config::GlobalConfig;
use autara_lib::state::market::Market;
use autara_program_lib::accounts::signer::Signer;
use autara_program_lib::accounts::zero_copy::{ZeroCopyOwnedAccount, ZeroCopyOwnedAccountMut};

use crate::error::{LendingAccountValidationError, LendingProgramResult};
use crate::state::AutaraAccount;

pub struct UpdateConfigAccounts<'a, 'b> {
    pub market: ZeroCopyOwnedAccountMut<'a, 'b, AutaraAccount<Market>>,
    pub global_config: ZeroCopyOwnedAccount<'a, 'b, AutaraAccount<GlobalConfig>>,
    pub curator: Signer<'a, 'b>,
    pub updated_supply_oracle: &'b AccountInfo<'a>,
    pub updated_collateral_oracle: &'b AccountInfo<'a>,
}

impl<'a, 'b> UpdateConfigAccounts<'a, 'b> {
    pub fn from_accounts(
        accounts: &mut impl Iterator<Item = &'b AccountInfo<'a>>,
    ) -> LendingProgramResult<Self>
    where
        'a: 'b,
    {
        let this = Self {
            market: next_account_info(accounts)?.try_into()?,
            global_config: next_account_info(accounts)?.try_into()?,
            curator: next_account_info(accounts)?.try_into()?,
            updated_supply_oracle: next_account_info(accounts)?,
            updated_collateral_oracle: next_account_info(accounts)?,
        };
        this.validate()?;
        Ok(this)
    }

    pub fn validate(&self) -> LendingProgramResult {
        let market = self.market.load_ref();
        if market.config().curator() != self.curator.key {
            return Err(LendingAccountValidationError::InvalidMarketAuthority.into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use autara_program_lib::accounts::AccountValidationError;

    use super::*;
    use crate::ixs::test_utils::AutaraAccounts;

    #[test]
    pub fn validate_correct_accounts() {
        let account_set = AutaraAccounts::new();
        let accounts = [
            account_set.market.clone(),
            account_set.global_config.clone(),
            account_set.curator.clone(),
            account_set.oracle.clone(),
            account_set.oracle.clone(),
        ];
        let accounts_iter = accounts.iter();
        UpdateConfigAccounts::from_accounts(&mut accounts_iter.into_iter()).unwrap();
    }

    #[test]
    pub fn validate_fails_if_curator_is_not_signer() {
        let mut account_set = AutaraAccounts::new();
        account_set.curator.non_signer();
        let accounts = [
            account_set.market.clone(),
            account_set.global_config.clone(),
            account_set.curator.clone(),
            account_set.oracle.clone(),
            account_set.oracle.clone(),
        ];
        let accounts_iter = accounts.iter();
        let result = UpdateConfigAccounts::from_accounts(&mut accounts_iter.into_iter());
        let Err(err) = result else {
            panic!("Expected an error, but got Ok");
        };
        assert_eq!(err, AccountValidationError::NotSigner);
    }

    #[test]
    pub fn validate_fails_if_market_is_not_owned_by_crate() {
        let mut account_set = AutaraAccounts::new();
        account_set.market.mutate_owner();
        let accounts = [
            account_set.market.clone(),
            account_set.global_config.clone(),
            account_set.curator.clone(),
            account_set.oracle.clone(),
            account_set.oracle.clone(),
        ];
        let accounts_iter = accounts.iter();
        let result = UpdateConfigAccounts::from_accounts(&mut accounts_iter.into_iter());
        let Err(err) = result else {
            panic!("Expected an error, but got Ok");
        };
        assert_eq!(err, AccountValidationError::InvalidOwner);
    }

    #[test]
    pub fn validate_fails_if_curator_mismatch() {
        let account_set_a = AutaraAccounts::new();
        let account_set_b = AutaraAccounts::new();
        let accounts = [
            account_set_b.market.clone(),
            account_set_a.global_config.clone(),
            account_set_a.curator.clone(),
            account_set_a.oracle.clone(),
            account_set_a.oracle.clone(),
        ];
        let accounts_iter = accounts.iter();
        let result = UpdateConfigAccounts::from_accounts(&mut accounts_iter.into_iter());
        let Err(err) = result else {
            panic!("Expected an error, but got Ok");
        };
        assert_eq!(err, LendingAccountValidationError::InvalidMarketAuthority);
    }
}
