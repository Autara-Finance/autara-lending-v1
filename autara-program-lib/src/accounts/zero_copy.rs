use std::{
    cell::{Ref, RefMut},
    ops::Deref,
};

use arch_program::account::AccountInfo;
use bytemuck::Pod;

use crate::accounts::{AccountValidationError, OwnedAccount};

pub struct ZeroCopyOwnedAccount<'a, 'b, T: OwnedAccount + Pod + ZeroCopyInitialized> {
    account: &'b AccountInfo<'a>,
    _marker: std::marker::PhantomData<T>,
}

impl<'a, 'b, T: OwnedAccount + Pod + ZeroCopyInitialized> ZeroCopyOwnedAccount<'a, 'b, T> {
    pub fn load_ref(&self) -> Ref<T> {
        let ref_ = self.account.data.try_borrow().unwrap();
        Ref::map(ref_, |data| bytemuck::from_bytes::<T>(data))
    }

    pub fn key(&self) -> &'b arch_program::pubkey::Pubkey {
        &self.account.key
    }

    pub fn is_signer(&self) -> bool {
        self.account.is_signer
    }
}

impl<'a, 'b, T: OwnedAccount + Pod + ZeroCopyInitialized> TryFrom<&'b AccountInfo<'a>>
    for ZeroCopyOwnedAccount<'a, 'b, T>
{
    type Error = AccountValidationError;

    fn try_from(value: &'b AccountInfo<'a>) -> Result<Self, Self::Error> {
        if !T::is_valid_owner(&value.owner) {
            return Err(AccountValidationError::InvalidOwner);
        }
        let ref_ = value
            .data
            .try_borrow()
            .map_err(|_| AccountValidationError::AlreadyLoaded)?;
        let this = bytemuck::try_from_bytes::<T>(&ref_)
            .map_err(|_| AccountValidationError::InvalidData)?;
        if !this.is_initialized() {
            return Err(AccountValidationError::AccountNotInitialized);
        }
        Ok(ZeroCopyOwnedAccount {
            account: value,
            _marker: std::marker::PhantomData,
        })
    }
}

pub struct ZeroCopyOwnedAccountMut<'a, 'b, T: OwnedAccount + Pod + ZeroCopyInitialized>(
    ZeroCopyOwnedAccount<'a, 'b, T>,
);

impl<'a, 'b, T: OwnedAccount + Pod + ZeroCopyInitialized> ZeroCopyOwnedAccountMut<'a, 'b, T> {
    pub fn load_mut(&self) -> RefMut<T> {
        let ref_ = self.0.account.data.try_borrow_mut().unwrap();
        RefMut::map(ref_, |data| bytemuck::from_bytes_mut::<T>(data))
    }
}

impl<'a, 'b, T: OwnedAccount + Pod + ZeroCopyInitialized> TryFrom<&'b AccountInfo<'a>>
    for ZeroCopyOwnedAccountMut<'a, 'b, T>
{
    type Error = AccountValidationError;

    fn try_from(value: &'b AccountInfo<'a>) -> Result<Self, Self::Error> {
        let account = ZeroCopyOwnedAccount::try_from(value)?;
        if !value.is_writable {
            return Err(AccountValidationError::NotWritable);
        }
        Ok(ZeroCopyOwnedAccountMut(account))
    }
}

impl<'a, 'b, T: OwnedAccount + Pod + ZeroCopyInitialized> Deref
    for ZeroCopyOwnedAccountMut<'a, 'b, T>
{
    type Target = ZeroCopyOwnedAccount<'a, 'b, T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub trait ZeroCopyInitialized {
    fn is_initialized(&self) -> bool;
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use arch_program::pubkey::Pubkey;
    use bytemuck::Zeroable;

    #[repr(C)]
    #[derive(Pod, Zeroable, Debug, Clone, Copy, PartialEq, Eq)]
    struct A {
        pub data: u64,
    }

    #[repr(C)]
    #[derive(Pod, Zeroable, Debug, Clone, Copy)]
    struct B {
        pub data: u128,
    }

    #[repr(C)]
    #[derive(Pod, Zeroable, Debug, Clone, Copy)]
    struct C {
        pub data: u64,
    }

    const OWNER: Pubkey = Pubkey([1; 32]);

    impl OwnedAccount for A {
        fn is_valid_owner(owner: &arch_program::pubkey::Pubkey) -> bool {
            *owner == OWNER
        }
    }

    impl ZeroCopyInitialized for A {
        fn is_initialized(&self) -> bool {
            self.data != 0
        }
    }

    impl OwnedAccount for B {
        fn is_valid_owner(owner: &arch_program::pubkey::Pubkey) -> bool {
            *owner == OWNER
        }
    }

    impl ZeroCopyInitialized for B {
        fn is_initialized(&self) -> bool {
            self.data != 0
        }
    }

    impl OwnedAccount for C {
        fn is_valid_owner(owner: &arch_program::pubkey::Pubkey) -> bool {
            *owner != OWNER
        }
    }

    impl ZeroCopyInitialized for C {
        fn is_initialized(&self) -> bool {
            self.data != 0
        }
    }

    pub fn create_pod<T>(data: T) -> AccountInfo<'static>
    where
        T: bytemuck::Pod,
    {
        let key = Box::leak(Box::new(Pubkey::new_unique()));
        let lamports = Box::leak(Box::new(1_000_000u64));
        let account_data = Box::leak(Box::new(bytemuck::bytes_of(&data).to_vec()));
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
    pub fn struct_a_cannot_be_loaded_as_struct_b() {
        let account_info = create_pod(A { data: 42 });
        let a_account = ZeroCopyOwnedAccount::<A>::try_from(&account_info);
        assert!(a_account.is_ok());
        let b_account: Result<ZeroCopyOwnedAccount<B>, _> =
            ZeroCopyOwnedAccount::try_from(&account_info);
        assert_eq!(
            b_account.map(|_| ()),
            Err(AccountValidationError::InvalidData)
        );
    }

    #[test]
    pub fn struct_b_cant_be_loaded_as_struct_b() {
        let account_info = create_pod(B { data: 42 });
        let a_account: Result<ZeroCopyOwnedAccount<A>, _> =
            ZeroCopyOwnedAccount::try_from(&account_info);
        assert_eq!(
            a_account.map(|_| ()),
            Err(AccountValidationError::InvalidData)
        );
        let b_account = ZeroCopyOwnedAccount::<B>::try_from(&account_info);
        assert!(b_account.is_ok());
    }

    #[test]
    pub fn cant_load_unitialised_account() {
        let account_info = create_pod(A { data: 0 });
        let a_account: Result<ZeroCopyOwnedAccount<A>, _> =
            ZeroCopyOwnedAccount::try_from(&account_info);
        assert_eq!(
            a_account.map(|_| ()),
            Err(AccountValidationError::AccountNotInitialized)
        );
    }

    #[test]
    pub fn owner_is_checked() {
        let account_info = create_pod(C { data: 42 });
        let c_account = ZeroCopyOwnedAccount::<C>::try_from(&account_info);
        assert_eq!(
            c_account.map(|_| ()),
            Err(AccountValidationError::InvalidOwner)
        );
    }

    #[test]
    pub fn check_load_ref() {
        let struct_a = A { data: 42 };
        let account_info = create_pod(struct_a.clone());
        let a_account = ZeroCopyOwnedAccount::<A>::try_from(&account_info).unwrap();
        assert_eq!(*a_account.load_ref(), struct_a);
    }

    #[test]
    pub fn check_load_mut() {
        let struct_a = A { data: 42 };
        let account_info = create_pod(struct_a.clone());
        let a_account = ZeroCopyOwnedAccountMut::<A>::try_from(&account_info).unwrap();
        assert_eq!(*a_account.load_mut(), struct_a);
        a_account.load_mut().data = 84;
        assert_eq!(a_account.load_ref().data, 84);
    }

    #[test]
    pub fn check_is_signer() {
        let struct_a = A { data: 42 };
        let mut account_info = create_pod(struct_a.clone());
        account_info.is_signer = true;
        let a_account = ZeroCopyOwnedAccount::<A>::try_from(&account_info).unwrap();
        assert!(a_account.is_signer());
    }

    #[test]
    pub fn check_key() {
        let struct_a = A { data: 42 };
        let account_info = create_pod(struct_a.clone());
        let a_account = ZeroCopyOwnedAccount::<A>::try_from(&account_info).unwrap();
        assert_eq!(a_account.key(), account_info.key);
    }
}
