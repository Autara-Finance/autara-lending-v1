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
    apply_price_update(
        oracle_data,
        signer.key,
        pyth_data,
        not_initialized,
        clock().unix_timestamp,
    )
}

/// Writes `pyth_data` into the oracle account, enforcing the authority
/// binding: the signer that creates a feed becomes its authority, and every
/// later update must be signed by that same authority.
pub fn apply_price_update(
    oracle_data: &mut PythPriceAccount,
    signer: &Pubkey,
    pyth_data: &PythPrice,
    just_created: bool,
    unix_timestamp: i64,
) -> Result<(), ProgramError> {
    if just_created {
        oracle_data.authority = *signer;
    } else if oracle_data.authority != *signer {
        return Err(ProgramError::IncorrectAuthority);
    }
    oracle_data.pyth_price = *pyth_data;
    oracle_data.pyth_price.price.publish_time = unix_timestamp;
    oracle_data.pyth_price.ema_price.publish_time = unix_timestamp;
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

#[cfg(test)]
mod tests {
    use super::*;
    use bytemuck::Zeroable;

    const CREATOR: Pubkey = Pubkey([1u8; 32]);
    const INTRUDER: Pubkey = Pubkey([2u8; 32]);

    fn price(id: [u8; 32], value: u64) -> PythPrice {
        let mut price = PythPrice::zeroed();
        price.id = id;
        price.price.price = value;
        price.ema_price.price = value;
        price
    }

    #[test]
    fn feed_creation_sets_authority() {
        let mut account = PythPriceAccount::zeroed();
        apply_price_update(&mut account, &CREATOR, &price([7u8; 32], 100), true, 1000).unwrap();
        assert_eq!(account.authority, CREATOR);
        assert_eq!(account.pyth_price.price.price, 100);
        assert_eq!(account.pyth_price.price.publish_time, 1000);
    }

    #[test]
    fn authorized_push_succeeds() {
        let mut account = PythPriceAccount::zeroed();
        apply_price_update(&mut account, &CREATOR, &price([7u8; 32], 100), true, 1000).unwrap();
        apply_price_update(&mut account, &CREATOR, &price([7u8; 32], 200), false, 2000).unwrap();
        assert_eq!(account.authority, CREATOR);
        assert_eq!(account.pyth_price.price.price, 200);
        assert_eq!(account.pyth_price.price.publish_time, 2000);
    }

    #[test]
    fn unauthorized_push_fails() {
        let mut account = PythPriceAccount::zeroed();
        apply_price_update(&mut account, &CREATOR, &price([7u8; 32], 100), true, 1000).unwrap();
        let err = apply_price_update(&mut account, &INTRUDER, &price([7u8; 32], 1), false, 2000)
            .unwrap_err();
        assert_eq!(err, ProgramError::IncorrectAuthority);
        // the rejected update must not have touched the stored price
        assert_eq!(account.authority, CREATOR);
        assert_eq!(account.pyth_price.price.price, 100);
        assert_eq!(account.pyth_price.price.publish_time, 1000);
    }
}
