use crate::event::AutaraEvent;
use arch_program::{account::AccountMeta, instruction::Instruction, pubkey::Pubkey};
use borsh::BorshSerialize;

use super::types::AurataInstruction;

pub fn log_ix(autara_program_id: &Pubkey, market: &Pubkey, event: AutaraEvent) -> Instruction {
    let mut data = Vec::new();
    AurataInstruction::Log.serialize(&mut data).unwrap();
    event.serialize(&mut data).unwrap();
    let accounts = vec![AccountMeta::new_readonly(*market, true)];
    Instruction {
        program_id: *autara_program_id,
        accounts,
        data,
    }
}
