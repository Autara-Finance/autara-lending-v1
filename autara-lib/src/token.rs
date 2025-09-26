use arch_program::{instruction::Instruction, pubkey::Pubkey, system_program::SYSTEM_PROGRAM_ID};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenInfo {
    pub mint: Pubkey,
    pub decimals: u8,
}

impl TokenInfo {
    pub fn get_associated_token_address(&self, owner: &Pubkey) -> Pubkey {
        get_associated_token_address(owner, &self.mint)
    }
}

pub fn create_ata_ix(
    funder_info: &Pubkey,
    associated_token_account_info: Option<&Pubkey>,
    owner_account_info: &Pubkey,
    spl_token_mint_info: &Pubkey,
) -> Instruction {
    let associated_token_account_info = if let Some(info) = associated_token_account_info {
        *info
    } else {
        get_associated_token_address(owner_account_info, spl_token_mint_info)
    };

    apl_associated_token_account::create_associated_token_account(
        funder_info,
        &associated_token_account_info,
        owner_account_info,
        spl_token_mint_info,
        &apl_token::id(),
        &SYSTEM_PROGRAM_ID,
    )
}

pub fn get_associated_token_address(
    wallet_address: &Pubkey,
    spl_token_mint_address: &Pubkey,
) -> Pubkey {
    apl_associated_token_account::get_associated_token_address_and_bump_seed(
        wallet_address,
        spl_token_mint_address,
        &apl_associated_token_account::id(),
    )
    .0
}
