use arch_program::pubkey::Pubkey;
use num_enum::{IntoPrimitive, TryFromPrimitive};

pub mod borsh;
pub mod packed;
pub mod program;
pub mod signer;
pub mod token;
pub mod zero_copy;

pub trait OwnedAccount {
    fn is_valid_owner(owner: &Pubkey) -> bool;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, IntoPrimitive, TryFromPrimitive)]
#[repr(u8)]
pub enum AccountValidationError {
    NotSigner,
    InvalidData,
    InvalidOwner,
    InvalidKey,
    AlreadyLoaded,
    NotWritable,
    AccountNotInitialized,
}
