use arch_program::{account::AccountMeta, instruction::Instruction, pubkey::Pubkey};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::pda::borrow_position_seed;

use super::types::AurataInstruction;

#[derive(Clone, Debug, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
#[repr(C)]
pub struct CreateBorrowPositionInstruction {
    pub bump: u8,
}

#[derive(Clone, Debug, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
#[repr(C)]
pub struct BorrowAplInstruction {
    pub amount: u64,
}

#[derive(Clone, Debug, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
#[repr(C)]
pub struct DepositAplCollateralInstruction {
    pub amount: u64,
}

#[derive(Clone, Debug, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
#[repr(C)]
pub struct WithdrawAplCollateralInstruction {
    pub amount: u64,
    pub withdraw_all: bool,
}

#[derive(Clone, Debug, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
#[repr(C)]
pub struct RepayAplInstruction {
    pub amount: u64,
    pub repay_all: bool,
}

#[derive(Clone, Debug, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
#[repr(C)]
#[cfg_attr(
    feature = "client",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct BorrowDepositAplInstruction {
    pub deposit_amount: u64,
    pub borrow_amount: u64,
    /// Optional callback instruction to be executed
    /// after borrowing supply and before depositing collateral
    /// Usefull for atomic leverage
    #[cfg_attr(feature = "client", serde(default))]
    pub ix_callback: Option<Instruction>,
}

#[derive(Clone, Debug, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
#[repr(C)]
#[cfg_attr(
    feature = "client",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct WithdrawRepayAplInstruction {
    pub repay_amount: u64,
    pub withdraw_amount: u64,
    pub repay_all: bool,
    pub withdraw_all: bool,
    /// Optional callback instruction to be executed
    /// after withdraing collateral and before repaying debt
    /// Usefull for atomic deleverage
    #[cfg_attr(feature = "client", serde(default))]
    pub ix_callback: Option<Instruction>,
}

pub fn create_borrow_position_ix(
    autara_program_id: Pubkey,
    market: Pubkey,
    authority: Pubkey,
    payer: Pubkey,
) -> (Pubkey, Instruction) {
    let borrow_seed = borrow_position_seed(&market, &authority);
    let (borrow_position, bump) = Pubkey::find_program_address(&borrow_seed, &autara_program_id);
    let mut data = Vec::new();
    AurataInstruction::CreateBorrowPosition(CreateBorrowPositionInstruction { bump })
        .serialize(&mut data)
        .unwrap();
    let accounts = vec![
        AccountMeta::new(market, false),
        AccountMeta::new(borrow_position, false),
        AccountMeta::new_readonly(authority, true),
        AccountMeta::new(payer, true),
        AccountMeta::new_readonly(arch_program::system_program::SYSTEM_PROGRAM_ID, false),
        AccountMeta::new_readonly(autara_program_id, false),
    ];
    (
        borrow_position,
        Instruction {
            program_id: autara_program_id,
            accounts,
            data,
        },
    )
}

pub fn borrow_apl_ix(
    autara_program_id: Pubkey,
    market: Pubkey,
    authority: Pubkey,
    borrow_position: Pubkey,
    authority_supply_ata: Pubkey,
    market_supply_vault: Pubkey,
    supply_oracle: Pubkey,
    collateral_oracle: Pubkey,
    amount: u64,
) -> Instruction {
    let mut data = Vec::new();
    AurataInstruction::BorrowApl(BorrowAplInstruction { amount })
        .serialize(&mut data)
        .unwrap();
    let accounts = vec![
        AccountMeta::new(market, false),
        AccountMeta::new(borrow_position, false),
        AccountMeta::new_readonly(authority, true),
        AccountMeta::new(authority_supply_ata, false),
        AccountMeta::new(market_supply_vault, false),
        AccountMeta::new_readonly(apl_token::id(), false),
        AccountMeta::new_readonly(supply_oracle, false),
        AccountMeta::new_readonly(collateral_oracle, false),
        AccountMeta::new_readonly(autara_program_id, false),
    ];
    Instruction {
        program_id: autara_program_id,
        accounts,
        data,
    }
}

pub fn deposit_apl_collateral_ix(
    autara_program_id: Pubkey,
    market: Pubkey,
    authority: Pubkey,
    borrow_position: Pubkey,
    authority_collateral_ata: Pubkey,
    market_collateral_vault: Pubkey,
    supply_oracle: Pubkey,
    collateral_oracle: Pubkey,
    amount: u64,
) -> Instruction {
    let mut data = Vec::new();
    AurataInstruction::DepositAplCollateral(DepositAplCollateralInstruction { amount })
        .serialize(&mut data)
        .unwrap();
    let accounts = vec![
        AccountMeta::new(market, false),
        AccountMeta::new(borrow_position, false),
        AccountMeta::new_readonly(authority, true),
        AccountMeta::new(authority_collateral_ata, false),
        AccountMeta::new(market_collateral_vault, false),
        AccountMeta::new_readonly(apl_token::id(), false),
        AccountMeta::new_readonly(supply_oracle, false),
        AccountMeta::new_readonly(collateral_oracle, false),
        AccountMeta::new_readonly(autara_program_id, false),
    ];
    Instruction {
        program_id: autara_program_id,
        accounts,
        data,
    }
}

pub fn withdraw_apl_collateral_ix(
    autara_program_id: Pubkey,
    market: Pubkey,
    authority: Pubkey,
    borrow_position: Pubkey,
    authority_collateral_ata: Pubkey,
    market_collateral_vault: Pubkey,
    supply_oracle: Pubkey,
    collateral_oracle: Pubkey,
    amount: u64,
    withdraw_all: bool,
) -> Instruction {
    let mut data = Vec::new();
    AurataInstruction::WithdrawAplCollateral(WithdrawAplCollateralInstruction {
        amount,
        withdraw_all,
    })
    .serialize(&mut data)
    .unwrap();
    let accounts = vec![
        AccountMeta::new(market, false),
        AccountMeta::new(borrow_position, false),
        AccountMeta::new_readonly(authority, true),
        AccountMeta::new(authority_collateral_ata, false),
        AccountMeta::new(market_collateral_vault, false),
        AccountMeta::new_readonly(apl_token::id(), false),
        AccountMeta::new_readonly(supply_oracle, false),
        AccountMeta::new_readonly(collateral_oracle, false),
        AccountMeta::new_readonly(autara_program_id, false),
    ];
    Instruction {
        program_id: autara_program_id,
        accounts,
        data,
    }
}

pub fn repay_apl_ix(
    autara_program_id: Pubkey,
    market: Pubkey,
    borrow_position: Pubkey,
    authority: Pubkey,
    authority_supply_ata: Pubkey,
    market_supply_vault: Pubkey,
    supply_oracle: Pubkey,
    collateral_oracle: Pubkey,
    amount: u64,
    repay_all: bool,
) -> Instruction {
    let mut data = Vec::new();
    AurataInstruction::RepayApl(RepayAplInstruction { amount, repay_all })
        .serialize(&mut data)
        .unwrap();
    let accounts = vec![
        AccountMeta::new(market, false),
        AccountMeta::new(borrow_position, false),
        AccountMeta::new_readonly(authority, true),
        AccountMeta::new(authority_supply_ata, false),
        AccountMeta::new(market_supply_vault, false),
        AccountMeta::new_readonly(apl_token::id(), false),
        AccountMeta::new_readonly(supply_oracle, false),
        AccountMeta::new_readonly(collateral_oracle, false),
        AccountMeta::new_readonly(autara_program_id, false),
    ];
    Instruction {
        program_id: autara_program_id,
        accounts,
        data,
    }
}

pub fn withdraw_repay_apl_ix(
    autara_program_id: Pubkey,
    market: Pubkey,
    authority: Pubkey,
    borrow_position: Pubkey,
    authority_supply_ata: Pubkey,
    market_supply_vault: Pubkey,
    authority_collateral_ata: Pubkey,
    market_collateral_vault: Pubkey,
    supply_oracle: Pubkey,
    collateral_oracle: Pubkey,
    ix: WithdrawRepayAplInstruction,
) -> Instruction {
    let mut accounts = vec![
        AccountMeta::new(market, false),
        AccountMeta::new(borrow_position, false),
        AccountMeta::new_readonly(authority, true),
        AccountMeta::new(authority_supply_ata, false),
        AccountMeta::new(market_supply_vault, false),
        AccountMeta::new(authority_collateral_ata, false),
        AccountMeta::new(market_collateral_vault, false),
        AccountMeta::new_readonly(apl_token::id(), false),
        AccountMeta::new_readonly(supply_oracle, false),
        AccountMeta::new_readonly(collateral_oracle, false),
        AccountMeta::new_readonly(autara_program_id, false),
    ];
    if let Some(ix_callback) = &ix.ix_callback {
        accounts.extend(ix_callback.accounts.iter().cloned());
    }
    let mut data = Vec::new();
    AurataInstruction::WithdrawRepayApl(ix)
        .serialize(&mut data)
        .unwrap();
    Instruction {
        program_id: autara_program_id,
        accounts,
        data,
    }
}

pub fn borrow_deposit_apl_ix(
    autara_program_id: Pubkey,
    market: Pubkey,
    authority: Pubkey,
    borrow_position: Pubkey,
    authority_supply_ata: Pubkey,
    market_supply_vault: Pubkey,
    authority_collateral_ata: Pubkey,
    market_collateral_vault: Pubkey,
    supply_oracle: Pubkey,
    collateral_oracle: Pubkey,
    ix: BorrowDepositAplInstruction,
) -> Instruction {
    let mut accounts = vec![
        AccountMeta::new(market, false),
        AccountMeta::new(borrow_position, false),
        AccountMeta::new_readonly(authority, true),
        AccountMeta::new(authority_supply_ata, false),
        AccountMeta::new(market_supply_vault, false),
        AccountMeta::new(authority_collateral_ata, false),
        AccountMeta::new(market_collateral_vault, false),
        AccountMeta::new_readonly(apl_token::id(), false),
        AccountMeta::new_readonly(supply_oracle, false),
        AccountMeta::new_readonly(collateral_oracle, false),
        AccountMeta::new_readonly(autara_program_id, false),
    ];
    if let Some(ix_callback) = &ix.ix_callback {
        accounts.extend(ix_callback.accounts.iter().cloned());
    }
    let mut data = Vec::new();
    AurataInstruction::BorrowDepositApl(ix)
        .serialize(&mut data)
        .unwrap();
    Instruction {
        program_id: autara_program_id,
        accounts,
        data,
    }
}
