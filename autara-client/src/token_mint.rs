use apl_token::state::Mint;
use arch_sdk::{
    arch_program::{program_pack::Pack, pubkey::Pubkey},
    AccountInfo, AccountInfoWithPubkey,
};
use autara_lib::token::{get_associated_token_address, TokenInfo};

pub struct TokenMint {
    mint: Pubkey,
    decimals: u8,
}

impl TokenMint {
    pub fn new(mint: Pubkey, decimals: u8) -> Self {
        Self { mint, decimals }
    }

    pub fn try_from_account_info(mint: Pubkey, account_info: &AccountInfo) -> anyhow::Result<Self> {
        if account_info.owner != apl_token::id() {
            return Err(anyhow::anyhow!("Account is not a token mint"));
        }
        let mint_acc = Mint::unpack(&account_info.data)?;
        Ok(Self {
            mint,
            decimals: mint_acc.decimals,
        })
    }

    pub fn try_from_account_info_with_pubkey(
        account_with_pubkey: &AccountInfoWithPubkey,
    ) -> anyhow::Result<Self> {
        if account_with_pubkey.owner != apl_token::id() {
            return Err(anyhow::anyhow!("Account is not a token mint"));
        }
        let mint_acc = Mint::unpack(&account_with_pubkey.data)?;
        Ok(Self {
            mint: account_with_pubkey.key,
            decimals: mint_acc.decimals,
        })
    }

    pub fn decimals(&self) -> u8 {
        self.decimals
    }

    pub fn mint(&self) -> Pubkey {
        self.mint
    }

    pub fn mint_ref(&self) -> &Pubkey {
        &self.mint
    }

    pub fn get_associated_token_account_address(&self, owner: &Pubkey) -> Pubkey {
        get_associated_token_address(owner, &self.mint)
    }
}

impl From<TokenInfo> for TokenMint {
    fn from(token_info: TokenInfo) -> Self {
        Self::new(token_info.mint, token_info.decimals)
    }
}
