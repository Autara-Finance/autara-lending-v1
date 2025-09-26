use std::ops::Deref;

use arch_program::{
    account::AccountInfo,
    program_pack::{IsInitialized, Pack},
};

use crate::accounts::{AccountValidationError, OwnedAccount};

pub struct PackedOwnedAccount<'a, 'b, T: OwnedAccount + Pack + IsInitialized> {
    account: &'b AccountInfo<'a>,
    inner: T,
}

impl<'a, 'b, T: OwnedAccount + Pack + IsInitialized> PackedOwnedAccount<'a, 'b, T> {
    pub fn key(&self) -> &'a arch_program::pubkey::Pubkey {
        &self.account.key
    }

    pub fn account_info(&self) -> &'b AccountInfo<'a> {
        self.account
    }

    pub fn reload(&mut self) -> Result<(), AccountValidationError> {
        *self = Self::try_from(self.account)?;
        Ok(())
    }
}

impl<'a, 'b, T: OwnedAccount + Pack + IsInitialized> Deref for PackedOwnedAccount<'a, 'b, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'a, 'b, T: OwnedAccount + Pack + IsInitialized> TryFrom<&'b AccountInfo<'a>>
    for PackedOwnedAccount<'a, 'b, T>
{
    type Error = AccountValidationError;

    fn try_from(value: &'b AccountInfo<'a>) -> Result<Self, Self::Error> {
        if !T::is_valid_owner(&value.owner) {
            return Err(AccountValidationError::InvalidOwner);
        }
        let inner = T::unpack(
            value
                .data
                .try_borrow()
                .map_err(|_| AccountValidationError::AlreadyLoaded)?
                .as_ref(),
        )
        .map_err(|_| AccountValidationError::InvalidData)?;
        Ok(PackedOwnedAccount {
            account: value,
            inner,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::OwnedAccount;
    use arch_program::program_error::ProgramError;
    use arch_program::program_pack::{IsInitialized, Pack, Sealed};
    use arch_program::pubkey::Pubkey;

    #[repr(C)]
    #[derive(Clone, Debug, PartialEq, Default)]
    pub struct A {
        pub data: u64,
        pub is_initialized: bool,
    }

    #[repr(C)]
    #[derive(Clone, Debug, PartialEq, Default)]
    pub struct B {
        pub data: u128,
        pub is_initialized: bool,
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

    impl IsInitialized for A {
        fn is_initialized(&self) -> bool {
            self.is_initialized
        }
    }

    impl IsInitialized for B {
        fn is_initialized(&self) -> bool {
            self.is_initialized
        }
    }

    impl Pack for A {
        const LEN: usize = 9;
        fn pack_into_slice(&self, dst: &mut [u8]) {
            dst[..8].copy_from_slice(&self.data.to_le_bytes());
            dst[8] = self.is_initialized as u8;
        }
        fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
            if src.len() < 9 {
                return Err(ProgramError::InvalidAccountData);
            }
            let mut data_bytes = [0u8; 8];
            data_bytes.copy_from_slice(&src[..8]);
            Ok(A {
                data: u64::from_le_bytes(data_bytes),
                is_initialized: src[8] != 0,
            })
        }
    }

    impl Pack for B {
        const LEN: usize = 17;
        fn pack_into_slice(&self, dst: &mut [u8]) {
            dst[..16].copy_from_slice(&self.data.to_le_bytes());
            dst[16] = self.is_initialized as u8;
        }
        fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
            if src.len() < 17 {
                return Err(ProgramError::InvalidAccountData);
            }
            let mut data_bytes = [0u8; 16];
            data_bytes.copy_from_slice(&src[..16]);
            Ok(B {
                data: u128::from_le_bytes(data_bytes),
                is_initialized: src[16] != 0,
            })
        }
    }

    const OWNER_2: Pubkey = Pubkey([2; 32]);

    #[repr(C)]
    #[derive(Clone, Debug, PartialEq, Default)]
    pub struct C {
        pub data: u64,
        pub is_initialized: bool,
    }

    impl OwnedAccount for C {
        fn is_valid_owner(owner: &arch_program::pubkey::Pubkey) -> bool {
            *owner == OWNER_2
        }
    }

    impl IsInitialized for C {
        fn is_initialized(&self) -> bool {
            self.is_initialized
        }
    }

    impl Pack for C {
        const LEN: usize = 9;
        fn pack_into_slice(&self, dst: &mut [u8]) {
            dst[..8].copy_from_slice(&self.data.to_le_bytes());
            dst[8] = self.is_initialized as u8;
        }
        fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
            if src.len() < 9 {
                return Err(ProgramError::InvalidAccountData);
            }
            let mut data_bytes = [0u8; 8];
            data_bytes.copy_from_slice(&src[..8]);
            Ok(C {
                data: u64::from_le_bytes(data_bytes),
                is_initialized: src[8] != 0,
            })
        }
    }

    impl Sealed for A {}
    impl Sealed for B {}
    impl Sealed for C {}

    pub fn create_packed_account<T>(data: T, owner: &Pubkey) -> AccountInfo<'static>
    where
        T: Pack,
    {
        let key = Box::leak(Box::new(Pubkey::new_unique()));
        let lamports = Box::leak(Box::new(1_000_000u64));
        let mut vec = vec![0u8; T::LEN];
        T::pack_into_slice(&data, &mut vec);
        let account_data = Box::leak(vec.into_boxed_slice());
        AccountInfo::new(
            key,
            lamports,
            account_data,
            Box::leak(Box::new(*owner)),
            Box::leak(Box::new(Default::default())),
            true,
            true,
            false,
        )
    }

    #[test]
    fn struct_a_cannot_be_loaded_as_struct_b() {
        let account_info = create_packed_account(
            A {
                data: 42,
                is_initialized: true,
            },
            &OWNER,
        );
        let a_account = PackedOwnedAccount::<A>::try_from(&account_info);
        assert!(a_account.is_ok());
        let b_account: Result<PackedOwnedAccount<B>, _> =
            PackedOwnedAccount::try_from(&account_info);
        assert_eq!(
            b_account.map(|_| ()),
            Err(AccountValidationError::InvalidData)
        );
    }

    #[test]
    fn struct_b_cannot_be_loaded_as_struct_a() {
        let account_info = create_packed_account(
            B {
                data: 42,
                is_initialized: true,
            },
            &OWNER,
        );
        let a_account: Result<PackedOwnedAccount<A>, _> =
            PackedOwnedAccount::try_from(&account_info);
        assert_eq!(
            a_account.map(|_| ()),
            Err(AccountValidationError::InvalidData)
        );
        let b_account = PackedOwnedAccount::<B>::try_from(&account_info);
        assert!(b_account.is_ok());
    }

    #[test]
    fn owner_is_checked() {
        let account_info = create_packed_account(
            C {
                data: 42,
                is_initialized: true,
            },
            &OWNER,
        );
        let c_account = PackedOwnedAccount::<C>::try_from(&account_info);
        assert_eq!(
            c_account.map(|_| ()),
            Err(AccountValidationError::InvalidOwner)
        );
    }
}
