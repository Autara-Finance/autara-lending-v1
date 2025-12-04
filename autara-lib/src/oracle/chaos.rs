use crate::constant::MAX_EXPONENT_ABS;
use crate::error::LendingResultExt;
use crate::{
    error::{LendingError, LendingResult},
    oracle::{
        oracle_price::OracleRate,
        oracle_provider::{AccountView, OracleLoader, UncheckedOracleRate},
    },
    padding::Padding,
};
use arch_program::pubkey::Pubkey;
use borsh::{BorshDeserialize, BorshSerialize};
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Debug, PartialEq, Eq, Copy, Clone, Pod, Zeroable, BorshSerialize, BorshDeserialize)]
#[cfg_attr(
    feature = "client",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct ChaosProvider {
    pub feed_id: [u8; 32],
    pub program_id: Pubkey,
    pub required_signatures: u8,
    pub pad: Padding<7>,
}

#[derive(Default, BorshDeserialize, BorshSerialize, Clone, Debug)]
pub struct ChaosPriceState {
    pub version: u16,
    /// Relation to ChaosPriceConfig id
    pub price_id: [u8; 32],
    /// Price
    pub price: PriceData,
    /// Ema price
    pub ema_price: PriceData,
    /// Count of valid signatures
    pub num_signatures: u8,
    /// Slot number of the transaction
    pub slot: u64,
    /// Timestamp when the data was posted
    pub publish_time: u64,
    pub bump: u8,
}

#[derive(Default, BorshDeserialize, BorshSerialize, Clone, Debug)]
pub struct PriceData {
    /// Price value
    pub price: u64,
    /// Timestamp of the observation
    pub observed_ts: u64,
    /// Exponent
    pub expo: i8,
}

// sha256("account:ChaosPriceState")[..8]
pub const CHAOS_DISCRIMINATOR: [u8; 8] = [168, 184, 18, 246, 179, 16, 141, 51];

pub const PRICE_CONFIG_SEED: &str = "chaos_price_config";

impl OracleLoader for ChaosProvider {
    fn load_oracle_price<D: std::ops::Deref<Target = [u8]>>(
        &self,
        view: AccountView<D>,
    ) -> LendingResult<UncheckedOracleRate> {
        if *view.owner != self.program_id {
            return Err(LendingError::InvalidChaosOracleAccount.into())
                .with_msg("Invalid program id");
        }
        let (disc, data) = view
            .data
            .split_at_checked(8)
            .ok_or_else(|| LendingError::InvalidChaosOracleAccount)?;
        if disc != &CHAOS_DISCRIMINATOR {
            return Err(LendingError::InvalidChaosOracleAccount.into())
                .with_msg("Invalid discriminator");
        }
        let chaos_price = ChaosPriceState::try_from_slice(data)
            .map_err(|_| LendingError::InvalidChaosOracleAccount)?;
        if chaos_price.price_id != self.feed_id {
            return Err(LendingError::InvalidOracleFeedId.into());
        }
        if chaos_price.num_signatures < self.required_signatures {
            return Err(LendingError::InvalidChaosOracleAccount.into())
                .with_msg("Insufficient signatures");
        }
        if (chaos_price.price.expo as i64) > MAX_EXPONENT_ABS
            || (chaos_price.price.expo as i64) < -MAX_EXPONENT_ABS
        {
            return Err(LendingError::InvalidChaosOracleAccount.into())
                .with_msg("Invalid exponent");
        }

        Ok(UncheckedOracleRate::new(
            OracleRate::try_from_price_expo_conf(
                chaos_price.price.price,
                0,
                chaos_price.price.expo,
            )?,
            chaos_price.price.observed_ts as _,
        ))
    }
}

#[cfg(test)]
pub mod tests {

    use arch_program::bitcoin::hashes::sha256;

    use super::*;

    #[test]
    fn check_discriminator() {
        assert_eq!(CHAOS_DISCRIMINATOR, [168, 184, 18, 246, 179, 16, 141, 51]);
        let hash = sha256::Hash::const_hash(b"account:ChaosPriceState");
        let hash: &[u8] = hash.as_ref();
        assert_eq!(&CHAOS_DISCRIMINATOR, &hash[..8]);
    }
}
