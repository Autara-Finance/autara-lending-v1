use arch_program::{account::AccountMeta, instruction::Instruction, pubkey::Pubkey};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::pda::supply_position_seed;

use super::types::AurataInstruction;

#[derive(Clone, Debug, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
#[repr(C)]
pub struct CreateSupplyPositionInstruction {
    pub bump: u8,
}

#[derive(Clone, Debug, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
#[repr(C)]
pub struct SupplyAplInstruction {
    pub amount: u64,
}

#[derive(Clone, Debug, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
#[repr(C)]
pub struct WithdrawSupplyInstruction {
    pub amount: u64,
    pub withdraw_all: bool,
}

#[derive(Clone, Debug, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
#[repr(C)]
pub struct DonateSupplyInstruction {
    pub amount: u64,
}

pub fn create_supply_position_ix(
    autara_program_id: Pubkey,
    market: Pubkey,
    authority: Pubkey,
    payer: Pubkey,
) -> (Pubkey, Instruction) {
    let supply_seed = supply_position_seed(&market, &authority);
    let (supply_position, bump) = Pubkey::find_program_address(&supply_seed, &autara_program_id);
    let mut data = Vec::new();
    AurataInstruction::CreateSupplyPosition(CreateSupplyPositionInstruction { bump })
        .serialize(&mut data)
        .unwrap();
    let accounts = vec![
        AccountMeta::new(market, false),
        AccountMeta::new(supply_position, false),
        AccountMeta::new_readonly(authority, true),
        AccountMeta::new(payer, true),
        AccountMeta::new_readonly(arch_program::system_program::SYSTEM_PROGRAM_ID, false),
        AccountMeta::new_readonly(autara_program_id, false),
    ];
    (
        supply_position,
        Instruction {
            program_id: autara_program_id,
            accounts,
            data,
        },
    )
}

pub fn supply_apl_ix(
    autara_program_id: Pubkey,
    market: Pubkey,
    supply_position: Pubkey,
    authority: Pubkey,
    authority_supply_ata: Pubkey,
    supply_vault: Pubkey,
    supply_oracle: Pubkey,
    collateral_oracle: Pubkey,
    amount: u64,
) -> Instruction {
    let mut data = Vec::new();
    AurataInstruction::SupplyApl(SupplyAplInstruction { amount })
        .serialize(&mut data)
        .unwrap();
    let accounts = vec![
        AccountMeta::new(market, false),
        AccountMeta::new(supply_position, false),
        AccountMeta::new_readonly(authority, true),
        AccountMeta::new(authority_supply_ata, false),
        AccountMeta::new(supply_vault, false),
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

pub fn withdraw_supply_ix(
    autara_program_id: Pubkey,
    market: Pubkey,
    supply_position: Pubkey,
    authority: Pubkey,
    authority_supply_ata: Pubkey,
    market_supply_vault: Pubkey,
    supply_oracle: Pubkey,
    collateral_oracle: Pubkey,
    amount: u64,
    withdraw_all: bool,
) -> Instruction {
    let mut data = Vec::new();
    AurataInstruction::WithdrawSupply(WithdrawSupplyInstruction {
        amount,
        withdraw_all,
    })
    .serialize(&mut data)
    .unwrap();
    let accounts = vec![
        AccountMeta::new(market, false),
        AccountMeta::new(supply_position, false),
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

pub fn donate_supply_ix(
    autara_program_id: Pubkey,
    market: Pubkey,
    authority: Pubkey,
    authority_supply_ata: Pubkey,
    supply_vault: Pubkey,
    amount: u64,
) -> Instruction {
    let mut data = Vec::new();
    AurataInstruction::DonateSupply(DonateSupplyInstruction { amount })
        .serialize(&mut data)
        .unwrap();
    let accounts = vec![
        AccountMeta::new(market, false),
        AccountMeta::new_readonly(authority, true),
        AccountMeta::new(authority_supply_ata, false),
        AccountMeta::new(supply_vault, false),
        AccountMeta::new_readonly(apl_token::id(), false),
        AccountMeta::new_readonly(autara_program_id, false),
    ];
    Instruction {
        program_id: autara_program_id,
        accounts,
        data,
    }
}
