use std::ops::Deref;

use arch_program::account::AccountInfo;

use crate::accounts::AccountValidationError;

pub struct Signer<'a, 'b> {
    account: &'b AccountInfo<'a>,
}

impl<'a, 'b> Deref for Signer<'a, 'b> {
    type Target = AccountInfo<'a>;

    fn deref(&self) -> &Self::Target {
        &self.account
    }
}

impl<'a, 'b> TryFrom<&'b AccountInfo<'a>> for Signer<'a, 'b> {
    type Error = AccountValidationError;

    fn try_from(account: &'b AccountInfo<'a>) -> Result<Self, Self::Error> {
        if account.is_signer {
            Ok(Signer { account })
        } else {
            Err(AccountValidationError::NotSigner)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arch_program::pubkey::Pubkey;

    pub fn create_account(is_signer: bool) -> AccountInfo<'static> {
        let key = Box::leak(Box::new(Pubkey::new_unique()));
        let lamports = Box::leak(Box::new(1_000_000u64));
        let account_data = Box::leak(Box::new([0u8; 0]));
        let owner = Box::leak(Box::new(Pubkey::new_unique()));
        AccountInfo::new(
            key,
            lamports,
            account_data,
            owner,
            Box::leak(Box::new(Default::default())),
            is_signer,
            false,
            false,
        )
    }

    #[test]
    fn signer_conversion_succeeds_when_account_is_signer() {
        let account_info = create_account(true);
        let signer_result = Signer::try_from(&account_info);
        assert!(signer_result.is_ok());
    }

    #[test]
    fn signer_conversion_fails_when_account_is_not_signer() {
        let account_info = create_account(false);
        let signer_result = Signer::try_from(&account_info);
        assert!(signer_result.is_err());
        assert_eq!(
            signer_result.err().unwrap(),
            AccountValidationError::NotSigner
        );
    }
}
