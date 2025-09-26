use std::ops::Deref;

use arch_program::account::AccountInfo;
use borsh::BorshDeserialize;

use crate::accounts::{AccountValidationError, OwnedAccount};

pub struct BorshOwnedAccount<'a, 'b, T: OwnedAccount + BorshDeserialize> {
    account: &'b AccountInfo<'a>,
    inner: T,
}

impl<'a, 'b, T: OwnedAccount + BorshDeserialize> BorshOwnedAccount<'a, 'b, T> {
    pub fn key(&self) -> &'a arch_program::pubkey::Pubkey {
        &self.account.key
    }

    pub fn reload(&mut self) -> Result<(), AccountValidationError> {
        *self = Self::try_from(self.account)?;
        Ok(())
    }
}

impl<'a, 'b, T: OwnedAccount + BorshDeserialize> Deref for BorshOwnedAccount<'a, 'b, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'a, 'b, T: OwnedAccount + BorshDeserialize> TryFrom<&'b AccountInfo<'a>>
    for BorshOwnedAccount<'a, 'b, T>
{
    type Error = AccountValidationError;

    fn try_from(value: &'b AccountInfo<'a>) -> Result<Self, Self::Error> {
        if !T::is_valid_owner(&value.owner) {
            return Err(AccountValidationError::InvalidOwner);
        }
        let inner = T::try_from_slice(
            value
                .data
                .try_borrow()
                .map_err(|_| AccountValidationError::AlreadyLoaded)?
                .as_ref(),
        )
        .map_err(|_| AccountValidationError::InvalidData)?;
        Ok(BorshOwnedAccount {
            account: value,
            inner,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arch_program::pubkey::Pubkey;
    use borsh::{BorshDeserialize, BorshSerialize};

    #[repr(C)]
    #[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
    struct A {
        pub data: u64,
    }

    #[repr(C)]
    #[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
    struct B {
        pub data: u128,
    }

    const OWNER: Pubkey = Pubkey([1; 32]);

    impl OwnedAccount for A {
        fn is_valid_owner(owner: &arch_program::pubkey::Pubkey) -> bool {
            *owner == OWNER
        }
    }

    impl OwnedAccount for B {
        fn is_valid_owner(owner: &arch_program::pubkey::Pubkey) -> bool {
            *owner == OWNER
        }
    }

    const OWNER_2: Pubkey = Pubkey([2; 32]);

    #[repr(C)]
    #[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
    pub struct C {
        pub data: u64,
    }

    impl OwnedAccount for C {
        fn is_valid_owner(owner: &arch_program::pubkey::Pubkey) -> bool {
            *owner == OWNER_2
        }
    }

    pub fn create_borsh_account<T>(data: T) -> AccountInfo<'static>
    where
        T: BorshSerialize,
    {
        let key = Box::leak(Box::new(Pubkey::new_unique()));
        let lamports = Box::leak(Box::new(1_000_000u64));
        let mut vec = vec![];
        data.serialize(&mut vec).unwrap();
        let account_data = Box::leak(vec.into_boxed_slice());
        AccountInfo::new(
            key,
            lamports,
            account_data,
            Box::leak(Box::new(OWNER)),
            Box::leak(Box::new(Default::default())),
            true,
            true,
            false,
        )
    }

    #[test]
    fn struct_a_cannot_be_loaded_as_struct_b() {
        let account_info = create_borsh_account(A { data: 42 });
        let a_account = BorshOwnedAccount::<A>::try_from(&account_info);
        assert!(a_account.is_ok());
        let b_account: Result<BorshOwnedAccount<B>, _> = BorshOwnedAccount::try_from(&account_info);
        assert_eq!(
            b_account.map(|_| ()),
            Err(AccountValidationError::InvalidData)
        );
    }

    #[test]
    fn struct_b_cannot_be_loaded_as_struct_a() {
        let account_info = create_borsh_account(B { data: 42 });
        let a_account: Result<BorshOwnedAccount<A>, _> = BorshOwnedAccount::try_from(&account_info);
        assert_eq!(
            a_account.map(|_| ()),
            Err(AccountValidationError::InvalidData)
        );
        let b_account = BorshOwnedAccount::<B>::try_from(&account_info);
        assert!(b_account.is_ok());
    }

    #[test]
    fn owner_is_checked() {
        let account_info = create_borsh_account(C { data: 42 });
        let c_account = BorshOwnedAccount::<C>::try_from(&account_info);
        assert_eq!(
            c_account.map(|_| ()),
            Err(AccountValidationError::InvalidOwner)
        );
    }

    #[test]
    fn check_deref() {
        let struct_a = A { data: 42 };
        let account_info = create_borsh_account(struct_a.clone());
        let a_account = BorshOwnedAccount::<A>::try_from(&account_info).unwrap();
        assert_eq!(*a_account, struct_a);
    }

    #[test]
    fn check_key() {
        let struct_a = A { data: 42 };
        let account_info = create_borsh_account(struct_a.clone());
        let a_account = BorshOwnedAccount::<A>::try_from(&account_info).unwrap();
        assert_eq!(a_account.key(), account_info.key);
    }

    #[test]
    fn check_reload() {
        let struct_a = A { data: 42 };
        let account_info = create_borsh_account(struct_a.clone());
        let mut a_account = BorshOwnedAccount::<A>::try_from(&account_info).unwrap();
        assert_eq!(*a_account, struct_a);
        let new_struct_a = A { data: 84 };
        let mut vec = vec![];
        new_struct_a.serialize(&mut vec).unwrap();
        let new_account_data = Box::leak(vec.into_boxed_slice());
        *account_info.data.try_borrow_mut().unwrap() = new_account_data;
        a_account.reload().unwrap();
        assert_eq!(*a_account, new_struct_a);
    }

    #[test]
    fn check_already_loaded() {
        let struct_a = A { data: 42 };
        let account_info = create_borsh_account(struct_a.clone());
        let _d = account_info.data.try_borrow_mut().unwrap();
        let err = BorshOwnedAccount::<A>::try_from(&account_info);
        assert_eq!(err.map(|_| ()), Err(AccountValidationError::AlreadyLoaded));
    }
}
