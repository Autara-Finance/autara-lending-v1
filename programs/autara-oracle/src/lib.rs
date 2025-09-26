use arch_program::{
    account::AccountInfo, program::invoke_signed_unchecked, program_error::ProgramError,
    pubkey::Pubkey, rent::minimum_rent, system_instruction,
};
use autara_lib::oracle::pyth::{PythPrice, PythPriceAccount};

#[cfg(feature = "entrypoint")]
arch_program::entrypoint!(process_instruction);

pub fn process_instruction<'a>(
    program_id: &Pubkey,
    accounts: &'a [AccountInfo<'a>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    if accounts.len() < 2 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    let signer = &accounts[0];
    let oracle = &accounts[1];
    if !signer.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    let pyth_data: &PythPrice = bytemuck::try_from_bytes(instruction_data)
        .map_err(|_| ProgramError::InvalidInstructionData)?;
    let not_initialized = oracle.owner != program_id;
    if not_initialized {
        let (_, bump) = Pubkey::find_program_address(&[&pyth_data.id], program_id);
        invoke_signed_unchecked(
            &system_instruction::create_account(
                signer.key,
                oracle.key,
                minimum_rent(std::mem::size_of::<PythPriceAccount>()),
                std::mem::size_of::<PythPriceAccount>() as u64,
                program_id,
            ),
            accounts,
            &[&[&pyth_data.id, &[bump]]],
        )?;
    }
    let mut oracle_bytes_mut = oracle.try_borrow_mut_data()?;
    let oracle_data: &mut PythPriceAccount = bytemuck::from_bytes_mut(&mut oracle_bytes_mut);
    let clock = clock();
    oracle_data.pyth_price = *pyth_data;
    oracle_data.pyth_price.price.publish_time = clock.unix_timestamp;
    oracle_data.pyth_price.ema_price.publish_time = clock.unix_timestamp;
    Ok(())
}

use arch_program::clock::Clock;

pub fn clock() -> Clock {
    let mut clock = Clock::default();
    unsafe { arch_program::syscalls::arch_get_clock(&mut clock) };
    if clock.unix_timestamp == 0 {
        panic!()
    }
    clock
}
