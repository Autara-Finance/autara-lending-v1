use arch_program::{
    account::{AccountInfo, AccountMeta},
    instruction::Instruction,
    program::invoke_signed_unchecked,
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::minimum_rent,
    system_instruction,
};
use autara_lib::oracle::pyth::{PythPrice, PythPriceAccount};
use std::mem::size_of;

#[cfg(feature = "entrypoint")]
arch_program::entrypoint!(process_instruction);

/// Discriminator for the authority-rotation instruction.
///
/// The price-update path stays tag-less for byte-compatibility with the
/// deployed program: any payload of exactly `size_of::<PythPrice>()` bytes
/// (120) is a price update, exactly as before. Tagged instructions are
/// 1 + 32 = 33 bytes and can therefore never collide with it (guarded by
/// `dispatch_is_unambiguous` below).
pub const UPDATE_AUTHORITY_TAG: u8 = 1;

/// Total length of an `UpdateAuthority` instruction payload: tag + pubkey.
pub const UPDATE_AUTHORITY_LEN: usize = 1 + 32;

pub fn process_instruction<'a>(
    program_id: &Pubkey,
    accounts: &'a [AccountInfo<'a>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    if instruction_data.len() == size_of::<PythPrice>() {
        process_price_update(program_id, accounts, instruction_data)
    } else if let Some(new_authority) = parse_update_authority(instruction_data) {
        process_update_authority(program_id, accounts, &new_authority)
    } else {
        Err(ProgramError::InvalidInstructionData)
    }
}

/// The original (deployed) instruction: create-if-needed + write the price.
/// Accounts: `[pusher (signer, writable), feed PDA (writable), system program]`.
fn process_price_update<'a>(
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
    let just_created = ensure_feed_account(program_id, accounts, signer, oracle, &pyth_data.id)?;
    let mut oracle_bytes_mut = oracle.try_borrow_mut_data()?;
    let oracle_data: &mut PythPriceAccount = bytemuck::from_bytes_mut(&mut oracle_bytes_mut);
    apply_price_update(
        oracle_data,
        signer.key,
        pyth_data,
        just_created,
        clock().unix_timestamp,
    )
}

/// Creates a new feed account, or migrates a pre-authority (120-byte) feed up to
/// `PythPriceAccount` size. Returns whether the caller should treat this as a
/// freshly created feed (bind `authority` to the signer).
fn ensure_feed_account<'a>(
    program_id: &Pubkey,
    accounts: &'a [AccountInfo<'a>],
    signer: &AccountInfo<'a>,
    oracle: &AccountInfo<'a>,
    feed_id: &[u8; 32],
) -> Result<bool, ProgramError> {
    let new_len = size_of::<PythPriceAccount>();
    if oracle.owner != program_id {
        let (_, bump) = Pubkey::find_program_address(&[feed_id], program_id);
        invoke_signed_unchecked(
            &system_instruction::create_account(
                signer.key,
                oracle.key,
                minimum_rent(new_len),
                new_len as u64,
                program_id,
            ),
            accounts,
            &[&[feed_id.as_ref(), &[bump]]],
        )?;
        return Ok(true);
    }

    // Legacy testnet feeds predate the trailing authority field and are exactly
    // one `PythPrice`. Grow in place on the next push; the pusher signer becomes
    // the feed authority (use a stable SIGNER_KEY_B64 before upgrading).
    if oracle.data_len() == size_of::<PythPrice>() {
        let required = minimum_rent(new_len);
        let current = oracle.lamports();
        if current < required {
            invoke_signed_unchecked(
                &system_instruction::transfer(signer.key, oracle.key, required - current),
                accounts,
                &[],
            )?;
        }
        oracle.realloc(new_len, true)?;
        return Ok(true);
    }

    if oracle.data_len() != new_len {
        return Err(ProgramError::InvalidAccountData);
    }
    Ok(false)
}

/// Authority rotation: hand an existing feed to a new pusher key.
/// Accounts: `[current authority (signer), feed PDA (writable)]`.
///
/// The feed must already exist (rotation cannot create feeds), and only the
/// currently pinned authority may rotate it. Rotating to the same key is a
/// no-op that still succeeds (idempotent re-runs of the rotation script).
fn process_update_authority<'a>(
    program_id: &Pubkey,
    accounts: &'a [AccountInfo<'a>],
    new_authority: &Pubkey,
) -> Result<(), ProgramError> {
    if accounts.len() < 2 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    let signer = &accounts[0];
    let oracle = &accounts[1];
    if !signer.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if oracle.owner != program_id {
        return Err(ProgramError::UninitializedAccount);
    }
    let mut oracle_bytes_mut = oracle.try_borrow_mut_data()?;
    let oracle_data: &mut PythPriceAccount = bytemuck::try_from_bytes_mut(&mut oracle_bytes_mut)
        .map_err(|_| ProgramError::InvalidAccountData)?;
    apply_authority_update(oracle_data, signer.key, new_authority)
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

/// Rotates the feed authority: only the currently pinned authority may hand
/// the feed to `new_authority`.
pub fn apply_authority_update(
    oracle_data: &mut PythPriceAccount,
    signer: &Pubkey,
    new_authority: &Pubkey,
) -> Result<(), ProgramError> {
    if oracle_data.authority != *signer {
        return Err(ProgramError::IncorrectAuthority);
    }
    oracle_data.authority = *new_authority;
    Ok(())
}

/// Serializes an `UpdateAuthority` payload: `[UPDATE_AUTHORITY_TAG, new_authority]`.
pub fn update_authority_instruction_data(new_authority: &Pubkey) -> [u8; UPDATE_AUTHORITY_LEN] {
    let mut data = [0u8; UPDATE_AUTHORITY_LEN];
    data[0] = UPDATE_AUTHORITY_TAG;
    data[1..].copy_from_slice(&new_authority.serialize());
    data
}

/// Parses an `UpdateAuthority` payload; `None` if the bytes are not one.
pub fn parse_update_authority(data: &[u8]) -> Option<Pubkey> {
    if data.len() != UPDATE_AUTHORITY_LEN || data[0] != UPDATE_AUTHORITY_TAG {
        return None;
    }
    Some(Pubkey::from_slice(&data[1..]))
}

/// Client helper: the full `UpdateAuthority` instruction for one feed,
/// signed by the feed's current authority.
pub fn update_authority_instruction(
    autara_oracle_program_id: &Pubkey,
    pyth_feed_id: [u8; 32],
    current_authority: &Pubkey,
    new_authority: &Pubkey,
) -> Instruction {
    let (feed_pda, _) = Pubkey::find_program_address(&[&pyth_feed_id], autara_oracle_program_id);
    Instruction {
        program_id: *autara_oracle_program_id,
        accounts: vec![
            AccountMeta::new_readonly(*current_authority, true),
            AccountMeta::new(feed_pda, false),
        ],
        data: update_authority_instruction_data(new_authority).to_vec(),
    }
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
    const NEW_AUTHORITY: Pubkey = Pubkey([3u8; 32]);

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

    /// Migrated legacy feeds call `apply_price_update` with `just_created=true`
    /// so the first post-upgrade pusher becomes authority.
    #[test]
    fn migrate_binds_authority_like_create() {
        let mut account = PythPriceAccount::zeroed();
        // Simulate preserved legacy price bytes + zeroed authority after realloc.
        account.pyth_price = price([7u8; 32], 50);
        apply_price_update(&mut account, &CREATOR, &price([7u8; 32], 75), true, 3000).unwrap();
        assert_eq!(account.authority, CREATOR);
        assert_eq!(account.pyth_price.price.price, 75);
        let err = apply_price_update(&mut account, &INTRUDER, &price([7u8; 32], 1), false, 4000)
            .unwrap_err();
        assert_eq!(err, ProgramError::IncorrectAuthority);
    }

    // --- authority rotation ---

    #[test]
    fn rotation_by_wrong_signer_is_rejected() {
        let mut account = PythPriceAccount::zeroed();
        apply_price_update(&mut account, &CREATOR, &price([7u8; 32], 100), true, 1000).unwrap();
        let err = apply_authority_update(&mut account, &INTRUDER, &NEW_AUTHORITY).unwrap_err();
        assert_eq!(err, ProgramError::IncorrectAuthority);
        assert_eq!(account.authority, CREATOR);
    }

    #[test]
    fn rotation_persists() {
        let mut account = PythPriceAccount::zeroed();
        apply_price_update(&mut account, &CREATOR, &price([7u8; 32], 100), true, 1000).unwrap();
        apply_authority_update(&mut account, &CREATOR, &NEW_AUTHORITY).unwrap();
        assert_eq!(account.authority, NEW_AUTHORITY);
        // the rotation must not have touched the stored price
        assert_eq!(account.pyth_price.price.price, 100);
        assert_eq!(account.pyth_price.price.publish_time, 1000);
    }

    #[test]
    fn rotation_to_same_key_is_idempotent() {
        let mut account = PythPriceAccount::zeroed();
        apply_price_update(&mut account, &CREATOR, &price([7u8; 32], 100), true, 1000).unwrap();
        apply_authority_update(&mut account, &CREATOR, &NEW_AUTHORITY).unwrap();
        // re-running the rotation with the new key targeting itself succeeds
        apply_authority_update(&mut account, &NEW_AUTHORITY, &NEW_AUTHORITY).unwrap();
        assert_eq!(account.authority, NEW_AUTHORITY);
    }

    /// The full cutover sequence: old key creates + pushes, rotates to the
    /// new (cosigner) key, after which the old key is locked out and the new
    /// key pushes normally.
    #[test]
    fn full_old_to_new_rotation() {
        let mut account = PythPriceAccount::zeroed();
        // old key creates the feed and keeps pushing
        apply_price_update(&mut account, &CREATOR, &price([7u8; 32], 100), true, 1000).unwrap();
        apply_price_update(&mut account, &CREATOR, &price([7u8; 32], 110), false, 2000).unwrap();
        // rotation, signed by the old key
        apply_authority_update(&mut account, &CREATOR, &NEW_AUTHORITY).unwrap();
        // old key is now rejected
        let err = apply_price_update(&mut account, &CREATOR, &price([7u8; 32], 1), false, 3000)
            .unwrap_err();
        assert_eq!(err, ProgramError::IncorrectAuthority);
        assert_eq!(account.pyth_price.price.price, 110);
        // old key cannot rotate back either
        let err = apply_authority_update(&mut account, &CREATOR, &CREATOR).unwrap_err();
        assert_eq!(err, ProgramError::IncorrectAuthority);
        // new key pushes normally
        apply_price_update(
            &mut account,
            &NEW_AUTHORITY,
            &price([7u8; 32], 120),
            false,
            4000,
        )
        .unwrap();
        assert_eq!(account.authority, NEW_AUTHORITY);
        assert_eq!(account.pyth_price.price.price, 120);
        assert_eq!(account.pyth_price.price.publish_time, 4000);
    }

    // --- instruction encoding / dispatch ---

    /// Byte-compatibility guard: a price-update payload can never be parsed
    /// as an UpdateAuthority payload and vice versa.
    #[test]
    fn dispatch_is_unambiguous() {
        assert_ne!(UPDATE_AUTHORITY_LEN, size_of::<PythPrice>());
        let update = update_authority_instruction_data(&NEW_AUTHORITY);
        assert_eq!(parse_update_authority(&update), Some(NEW_AUTHORITY));
        // a price payload is not an authority update
        let price = price([7u8; 32], 100);
        assert_eq!(parse_update_authority(bytemuck::bytes_of(&price)), None);
        // wrong tag or wrong length are rejected
        let mut bad_tag = update;
        bad_tag[0] = 0xff;
        assert_eq!(parse_update_authority(&bad_tag), None);
        assert_eq!(parse_update_authority(&update[..32]), None);
    }

    #[test]
    fn update_authority_instruction_shape() {
        let program_id = Pubkey([9u8; 32]);
        let ix = update_authority_instruction(&program_id, [7u8; 32], &CREATOR, &NEW_AUTHORITY);
        assert_eq!(ix.program_id, program_id);
        assert_eq!(ix.accounts.len(), 2);
        // current authority signs, feed PDA is writable
        assert_eq!(ix.accounts[0].pubkey, CREATOR);
        assert!(ix.accounts[0].is_signer);
        assert!(!ix.accounts[0].is_writable);
        let (feed_pda, _) = Pubkey::find_program_address(&[&[7u8; 32]], &program_id);
        assert_eq!(ix.accounts[1].pubkey, feed_pda);
        assert!(!ix.accounts[1].is_signer);
        assert!(ix.accounts[1].is_writable);
        assert_eq!(parse_update_authority(&ix.data), Some(NEW_AUTHORITY));
    }
}
