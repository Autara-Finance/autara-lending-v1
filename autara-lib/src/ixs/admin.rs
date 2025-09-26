use arch_program::{account::AccountMeta, instruction::Instruction, pubkey::Pubkey};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{ixs::AurataInstruction, pda::find_global_config_pda};

#[repr(C)]
#[derive(Clone, Debug, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub struct CreateGlobalConfigInstruction {
    pub bump: u8,
    pub admin: Pubkey,
    pub fee_receiver: Pubkey,
    pub protocol_fee_share_in_bps: u16,
}

#[repr(C)]
#[derive(Clone, Debug, BorshSerialize, BorshDeserialize, PartialEq, Eq, Default)]
#[cfg_attr(
    feature = "client",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct UpdateGlobalConfigInstruction {
    pub accept_nomination: bool,
    #[cfg_attr(feature = "client", serde(default))]
    pub nominated_admin: Option<Pubkey>,
    #[cfg_attr(feature = "client", serde(default))]
    pub fee_receiver: Option<Pubkey>,
    #[cfg_attr(feature = "client", serde(default))]
    pub protocol_fee_share_in_bps: Option<u16>,
}

pub fn create_global_config_ix(
    autara_program_id: Pubkey,
    payer: Pubkey,
    admin: Pubkey,
    fee_receiver: Pubkey,
    protocol_fee_share_in_bps: u16,
) -> (Pubkey, Instruction) {
    let mut data = Vec::new();
    let (global_config_pda, bump) = find_global_config_pda(&autara_program_id);
    let ix = AurataInstruction::CreateGlobalConfig(CreateGlobalConfigInstruction {
        bump,
        admin,
        fee_receiver,
        protocol_fee_share_in_bps,
    });
    ix.serialize(&mut data).unwrap();
    let accounts = vec![
        AccountMeta::new(payer, true),
        AccountMeta::new(global_config_pda, false),
        AccountMeta::new_readonly(arch_program::system_program::SYSTEM_PROGRAM_ID, false),
        AccountMeta::new_readonly(autara_program_id, false),
    ];
    (
        global_config_pda,
        Instruction {
            program_id: autara_program_id,
            accounts,
            data,
        },
    )
}

pub fn update_global_config_ix(
    autara_program_id: Pubkey,
    admin: Pubkey,
    update: UpdateGlobalConfigInstruction,
) -> Instruction {
    let mut data = Vec::new();
    let (global_config_pda, _) = find_global_config_pda(&autara_program_id);
    AurataInstruction::UpdateGlobalConfig(update)
        .serialize(&mut data)
        .unwrap();
    let accounts = vec![
        AccountMeta::new(admin, true),
        AccountMeta::new(global_config_pda, false),
        AccountMeta::new_readonly(autara_program_id, false),
    ];
    Instruction {
        program_id: autara_program_id,
        accounts,
        data,
    }
}

pub fn reedeem_protocol_fees_ix(
    autara_program_id: Pubkey,
    market: Pubkey,
    admin: Pubkey,
    protocol_fee_receiver: Pubkey,
    market_vault: Pubkey,
) -> Instruction {
    let mut data = Vec::new();
    let (global_config_pda, _) = find_global_config_pda(&autara_program_id);
    AurataInstruction::ReedeemProtocolFees
        .serialize(&mut data)
        .unwrap();
    let accounts = vec![
        AccountMeta::new(admin, true),
        AccountMeta::new_readonly(global_config_pda, false),
        AccountMeta::new(market, false),
        AccountMeta::new(protocol_fee_receiver, false),
        AccountMeta::new(market_vault, false),
        AccountMeta::new_readonly(apl_token::id(), false),
        AccountMeta::new_readonly(autara_program_id, false),
    ];
    Instruction {
        program_id: autara_program_id,
        accounts,
        data,
    }
}
