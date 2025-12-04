use std::ops::Deref;

use arch_program::{account::AccountInfo, pubkey::Pubkey};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::{LendingError, LendingResult},
    oracle::{
        chaos::{ChaosProvider, PRICE_CONFIG_SEED},
        oracle_config::OracleValidationConfig,
        oracle_price::OracleRate,
        pyth::PythProvider,
    },
};

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
#[cfg_attr(
    feature = "client",
    derive(serde::Serialize, serde::Deserialize),
    serde(tag = "type", content = "content")
)]
pub enum OracleProvider {
    Pyth(PythProvider),
    Chaos(ChaosProvider),
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
#[cfg_attr(
    feature = "client",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct UncheckedOracleRate {
    rate: OracleRate,
    publish_time: i64,
}

impl UncheckedOracleRate {
    pub fn new(rate: OracleRate, publish_time: i64) -> Self {
        Self { rate, publish_time }
    }

    pub fn unsafe_rate(&self) -> OracleRate {
        self.rate
    }

    pub fn validate(
        &self,
        config: &OracleValidationConfig,
        unix_timestamp: i64,
    ) -> LendingResult<OracleRate> {
        if self.rate.rate().is_negative() || self.rate.confidence().is_negative() {
            return Err(LendingError::NegativeOracleRate.into());
        }
        if self.rate.rate().is_zero() {
            return Err(LendingError::OracleRateIsNull.into());
        }
        let age = unix_timestamp
            .checked_sub(self.publish_time)
            .ok_or(LendingError::SubtractionOverflow)?
            .max(0) as u64;
        if config.max_age().is_some_and(|max_age| age > max_age) {
            return Err(LendingError::OracleRateTooOld.into());
        }
        let relative_confidence = self.rate.relative_confidence()?;
        if config
            .min_relative_confidence()
            .is_some_and(|min_conf| &relative_confidence > min_conf)
        {
            return Err(LendingError::OracleRateRelativeConfidenceTooLow.into());
        }
        Ok(self.rate)
    }
}

pub trait OracleLoader {
    fn load_oracle_price<D: Deref<Target = [u8]>>(
        &self,
        view: AccountView<D>,
    ) -> LendingResult<UncheckedOracleRate>;
}

impl OracleProvider {
    pub fn as_ref<'a>(&'a self) -> OracleProviderRef<'a> {
        match self {
            OracleProvider::Pyth(provider) => OracleProviderRef::Pyth(provider),
            OracleProvider::Chaos(provider) => OracleProviderRef::Chaos(provider),
        }
    }
}

pub enum OracleProviderRef<'a> {
    Pyth(&'a PythProvider),
    Chaos(&'a ChaosProvider),
}

impl<'a> OracleProviderRef<'a> {
    pub fn oracle_feed_pubkey(&self) -> Option<Pubkey> {
        match self {
            OracleProviderRef::Pyth(provider) => {
                Some(Pubkey::find_program_address(&[&provider.feed_id], &provider.program_id).0)
            }
            OracleProviderRef::Chaos(provider) => Some(
                Pubkey::find_program_address(
                    &[PRICE_CONFIG_SEED.as_bytes(), &provider.feed_id],
                    &provider.program_id,
                )
                .0,
            ),
        }
    }
}

impl<'a> OracleLoader for OracleProviderRef<'a> {
    fn load_oracle_price<D: Deref<Target = [u8]>>(
        &self,
        view: AccountView<D>,
    ) -> LendingResult<UncheckedOracleRate> {
        match self {
            OracleProviderRef::Pyth(pyth_provider) => pyth_provider.load_oracle_price(view),
            OracleProviderRef::Chaos(chaos_provider) => chaos_provider.load_oracle_price(view),
        }
    }
}

#[derive(Clone, Copy)]
pub struct AccountView<'a, D> {
    pub key: &'a Pubkey,
    pub data: D,
    pub owner: &'a Pubkey,
}

impl<'a, 'b> TryFrom<&'b AccountInfo<'a>> for AccountView<'a, RefWrapper<'a, 'b>>
where
    'a: 'b,
{
    type Error = LendingError;

    fn try_from(
        account_info: &'b AccountInfo<'a>,
    ) -> Result<AccountView<'a, RefWrapper<'a, 'b>>, Self::Error> {
        Ok(AccountView {
            key: account_info.key,
            data: RefWrapper(
                account_info
                    .data
                    .try_borrow()
                    .map_err(|_| LendingError::FailedToLoadAccount)?,
            ),
            owner: account_info.owner,
        })
    }
}

pub struct RefWrapper<'a, 'b>(std::cell::Ref<'b, &'a mut [u8]>);

impl<'a, 'b> Deref for RefWrapper<'a, 'b> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &*self.0
    }
}

impl<'a, D: Deref<Target = [u8]>> From<(&'a Pubkey, D, &'a Pubkey)> for AccountView<'a, D> {
    fn from((key, data, owner): (&'a Pubkey, D, &'a Pubkey)) -> Self {
        AccountView { key, owner, data }
    }
}

#[cfg(feature = "client")]
pub mod client {
    use crate::oracle::oracle_provider::AccountView;
    use arch_sdk::AccountInfoWithPubkey;

    impl<'a> From<&'a AccountInfoWithPubkey> for AccountView<'a, &'a [u8]> {
        fn from(info: &'a AccountInfoWithPubkey) -> Self {
            AccountView {
                key: &info.key,
                data: &info.data,
                owner: &info.owner,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        error::LendingError, math::ifixed_point::IFixedPoint, oracle::oracle_price::OracleRate,
    };

    #[test]
    fn test_validate_valid_rate() {
        let rate = OracleRate::new(IFixedPoint::lit("1.5"), IFixedPoint::lit("0.01"));
        let oracle_rate = UncheckedOracleRate::new(rate, 100);
        let config = OracleValidationConfig::new(60, 0.05.into());
        let result = oracle_rate.validate(&config, 120);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), rate);
    }

    #[test]
    fn test_validate_rate_too_old() {
        let rate = OracleRate::new(IFixedPoint::lit("1.5"), IFixedPoint::lit("0.01"));
        let oracle_rate = UncheckedOracleRate::new(rate, 100);
        let config = OracleValidationConfig::new(10, 0.05.into());
        let result = oracle_rate.validate(&config, 120);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), LendingError::OracleRateTooOld);
    }

    #[test]
    fn test_validate_confidence_too_low() {
        let rate = OracleRate::new(IFixedPoint::lit("1.5"), IFixedPoint::lit("0.1"));
        let oracle_rate = UncheckedOracleRate::new(rate, 100);
        let config = OracleValidationConfig::new(60, 0.05.into());
        let result = oracle_rate.validate(&config, 120);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            LendingError::OracleRateRelativeConfidenceTooLow
        );
    }

    #[test]
    fn test_validate_negative_rate() {
        let rate = OracleRate::new(IFixedPoint::lit("-1.5"), IFixedPoint::lit("0.01"));
        let oracle_rate = UncheckedOracleRate::new(rate, 100);
        let config = OracleValidationConfig::new(60, 0.05.into());
        let result = oracle_rate.validate(&config, 120);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), LendingError::NegativeOracleRate);
    }

    #[test]
    fn test_validate_negative_confidence() {
        let rate = OracleRate::new(IFixedPoint::lit("1.5"), IFixedPoint::lit("-0.01"));
        let oracle_rate = UncheckedOracleRate::new(rate, 100);
        let config = OracleValidationConfig::new(60, 0.05.into());
        let result = oracle_rate.validate(&config, 120);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), LendingError::NegativeOracleRate);
    }
}
