use std::ops::Deref;

use arch_program::{account::AccountInfo, pubkey::Pubkey};

use crate::accounts::AccountValidationError;

pub trait ProgramAccount {
    fn is_valid_key(key: &Pubkey) -> bool;
}

pub struct Program<'a, 'b, T: ProgramAccount> {
    account: &'b AccountInfo<'a>,
    _marker: std::marker::PhantomData<T>,
}

impl<'a, 'b, T: ProgramAccount> Deref for Program<'a, 'b, T> {
    type Target = AccountInfo<'a>;

    fn deref(&self) -> &Self::Target {
        &self.account
    }
}

impl<'a, 'b, T: ProgramAccount> TryFrom<&'b AccountInfo<'a>> for Program<'a, 'b, T> {
    type Error = AccountValidationError;

    fn try_from(account: &'b AccountInfo<'a>) -> Result<Self, Self::Error> {
        if !T::is_valid_key(&account.key) {
            return Err(AccountValidationError::InvalidKey);
        }
        Ok(Program {
            account,
            _marker: std::marker::PhantomData,
        })
    }
}

pub struct SystemProgram;

impl ProgramAccount for SystemProgram {
    fn is_valid_key(key: &Pubkey) -> bool {
        *key == Pubkey::system_program()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestProgram;

    const TEST_PROGRAM_KEY: Pubkey = Pubkey([42u8; 32]);

    impl ProgramAccount for TestProgram {
        fn is_valid_key(key: &Pubkey) -> bool {
            *key == TEST_PROGRAM_KEY
        }
    }

    fn create_account(key: Pubkey) -> AccountInfo<'static> {
        let key = Box::leak(Box::new(key));
        let lamports = Box::leak(Box::new(1_000_000u64));
        let account_data = Box::leak(Box::new([0u8; 0]));
        let owner = Box::leak(Box::new(Pubkey::new_unique()));
        AccountInfo::new(
            key,
            lamports,
            account_data,
            owner,
            Box::leak(Box::new(Default::default())),
            true,
            true,
            false,
        )
    }

    #[test]
    fn program_conversion_succeeds_with_valid_key() {
        let account_info = create_account(TEST_PROGRAM_KEY);
        let program_result = Program::<TestProgram>::try_from(&account_info);
        assert!(program_result.is_ok());
    }

    #[test]
    fn program_conversion_fails_with_invalid_key() {
        let invalid_key = Pubkey([24u8; 32]);
        let account_info = create_account(invalid_key);
        let program_result = Program::<TestProgram>::try_from(&account_info);
        assert!(program_result.is_err());
        assert_eq!(
            program_result.err().unwrap(),
            AccountValidationError::InvalidKey
        );
    }

    #[test]
    fn system_program_validation_works() {
        let system_key = Pubkey::system_program();
        let account_info = create_account(system_key);
        let program_result = Program::<SystemProgram>::try_from(&account_info);
        assert!(program_result.is_ok());
    }
}
