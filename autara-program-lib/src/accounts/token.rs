use {
    crate::accounts::{packed::PackedOwnedAccount, program::ProgramAccount, OwnedAccount},
    apl_token::state::{Account, Mint},
    arch_program::{
        program_pack::{IsInitialized, Pack, Sealed},
        pubkey::Pubkey,
    },
    std::ops::Deref,
};

pub struct AplTokenProgram;

impl ProgramAccount for AplTokenProgram {
    fn is_valid_key(key: &Pubkey) -> bool {
        key == &apl_token::id()
    }
}

pub struct AplAssociatedTokenProgram;

impl ProgramAccount for AplAssociatedTokenProgram {
    fn is_valid_key(key: &Pubkey) -> bool {
        key == &apl_associated_token_account::id()
    }
}

impl OwnedAccount for Mint {
    fn is_valid_owner(owner: &Pubkey) -> bool {
        owner == &apl_token::id()
    }
}

impl OwnedAccount for Account {
    fn is_valid_owner(owner: &Pubkey) -> bool {
        owner == &apl_token::id()
    }
}

pub type TokenAccount = apl_token::state::Account;

pub struct BoxedTokenAccount(pub Box<apl_token::state::Account>);

impl Pack for BoxedTokenAccount {
    const LEN: usize = apl_token::state::Account::LEN;

    fn pack_into_slice(&self, dst: &mut [u8]) {
        self.0.pack_into_slice(dst)
    }

    fn unpack_from_slice(src: &[u8]) -> Result<Self, arch_program::program_error::ProgramError> {
        apl_token::state::Account::unpack_from_slice(src).map(|x| Self(Box::new(x)))
    }
}

impl IsInitialized for BoxedTokenAccount {
    fn is_initialized(&self) -> bool {
        self.0.is_initialized()
    }
}

impl OwnedAccount for BoxedTokenAccount {
    fn is_valid_owner(owner: &Pubkey) -> bool {
        TokenAccount::is_valid_owner(owner)
    }
}

impl Sealed for BoxedTokenAccount {}

impl<'a, 'b> PackedOwnedAccount<'a, 'b, TokenAccount> {
    pub fn token_amount(&self) -> Option<u64> {
        let data = self.account_info().try_borrow_data().ok()?;
        let bytes = data.get(64..72)?;
        Some(u64::from_le_bytes(bytes.try_into().ok()?))
    }
}

impl<'a, 'b> PackedOwnedAccount<'a, 'b, BoxedTokenAccount> {
    pub fn token_amount(&self) -> Option<u64> {
        let data = self.account_info().try_borrow_data().ok()?;
        let bytes = data.get(64..72)?;
        Some(u64::from_le_bytes(bytes.try_into().ok()?))
    }
}

impl Deref for BoxedTokenAccount {
    type Target = TokenAccount;

    fn deref(&self) -> &Self::Target {
        &*self.0
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use apl_token::state::AccountState;
    use arch_program::{program_pack::Pack, pubkey::Pubkey};

    #[test]
    fn test_token_account() {
        let key = Pubkey::new_unique();
        let mint = Pubkey::new_unique();
        let owner = Pubkey::new_unique();
        let account = TokenAccount {
            mint,
            owner,
            amount: 12345678,
            state: AccountState::Initialized,
            ..Default::default()
        };
        let mut lp = 0;
        let mut data = [0; apl_token::state::Account::LEN];
        let uxto = Default::default();
        TokenAccount::pack(account, &mut data).unwrap();
        let owner = apl_token::id();
        let account_info = arch_program::account::AccountInfo::new(
            &key, &mut lp, &mut data, &owner, &uxto, false, false, false,
        );
        let packed_account = PackedOwnedAccount::<TokenAccount>::try_from(&account_info).unwrap();
        assert_eq!(packed_account.token_amount(), Some(12345678));
    }
}
