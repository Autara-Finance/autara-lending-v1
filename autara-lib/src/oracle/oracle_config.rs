use borsh::{BorshDeserialize, BorshSerialize};
use bytemuck::{Pod, Zeroable};

use crate::{
    error::LendingResult,
    math::ifixed_point::IFixedPoint,
    oracle::{
        oracle_price::OracleRate,
        oracle_provider::{AccountView, OracleLoader},
        pod_oracle_provider::PodOracleProvider,
    },
    padding::Padding,
    pod_option::PodOption,
};

crate::validate_struct!(OracleConfig, 264);

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
#[cfg_attr(
    feature = "client",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct OracleConfig {
    /// Kind of oracle
    oracle_provider: PodOracleProvider,
    /// Config to sanitize oracle feed
    validation_config: OracleValidationConfig,
    pad: Padding<152>,
}

impl OracleConfig {
    pub fn new(
        oracle_provider: impl Into<PodOracleProvider>,
        validation_config: OracleValidationConfig,
    ) -> Self {
        Self {
            oracle_provider: oracle_provider.into(),
            validation_config,
            pad: Padding::default(),
        }
    }

    pub fn new_pyth(feed_id: [u8; 32], program_id: arch_program::pubkey::Pubkey) -> Self {
        Self {
            oracle_provider: PodOracleProvider::from_oracle_provider(
                crate::oracle::oracle_provider::OracleProvider::Pyth(
                    crate::oracle::pyth::PythProvider {
                        feed_id,
                        program_id,
                    },
                ),
            ),
            validation_config: OracleValidationConfig::default(),
            pad: Padding::default(),
        }
    }

    pub fn oracle_provider(&self) -> &PodOracleProvider {
        &self.oracle_provider
    }

    pub fn load_and_validate_oracle_rate<D: std::ops::Deref<Target = [u8]>>(
        &self,
        view: AccountView<D>,
        unix_timestamp: i64,
    ) -> LendingResult<OracleRate> {
        let unchecked_price = self.oracle_provider.load_oracle_price(view)?;
        unchecked_price.validate(&self.validation_config, unix_timestamp)
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
#[cfg_attr(
    feature = "client",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct OracleValidationConfig {
    /// Max age in seconds from which an oracle is considered as stale
    max_age: PodOption<u64>,
    /// Level from which relative confidence should stay under.
    /// Ex: If price is 150 +/- 1.6, relative confidence is 1.6/150 ~ 1.06%
    min_relative_confidence: PodOption<IFixedPoint>,
    /// Minimum number of signatures required for an oracle rate to be considered valid.
    min_signature_threshold: PodOption<u64>,
}

impl Default for OracleValidationConfig {
    fn default() -> Self {
        Self {
            max_age: PodOption::new(60 * 60),
            min_relative_confidence: PodOption::new(IFixedPoint::lit("0.05")),
            min_signature_threshold: PodOption::new(0),
        }
    }
}

impl OracleValidationConfig {
    pub fn new(max_age: u64, min_relative_confidence: IFixedPoint) -> Self {
        Self {
            max_age: PodOption::new(max_age),
            min_relative_confidence: PodOption::new(min_relative_confidence),
            min_signature_threshold: PodOption::new(0),
        }
    }

    pub fn max_age(&self) -> Option<u64> {
        self.max_age.as_ref().copied()
    }

    pub fn min_relative_confidence(&self) -> Option<&IFixedPoint> {
        self.min_relative_confidence.as_ref()
    }
}

#[cfg(test)]
pub mod tests {
    use arch_program::pubkey::Pubkey;

    use crate::{
        math::ifixed_point::IFixedPoint,
        oracle::{oracle_price::OracleRate, oracle_provider::OracleProvider, pyth::PythProvider},
    };

    use super::*;

    pub const BTC_FEED_ID: [u8; 32] = [1; 32];
    pub const USD_FEED_ID: [u8; 32] = [2; 32];

    pub fn default_btc_oracle_rate() -> OracleRate {
        OracleRate::new(
            IFixedPoint::from_num(100_000.0),
            IFixedPoint::from_num(100.0),
        )
    }

    pub fn btc_oracle_config() -> OracleConfig {
        OracleConfig::new(
            OracleProvider::Pyth(PythProvider {
                feed_id: BTC_FEED_ID,
                program_id: Pubkey(BTC_FEED_ID),
            }),
            Default::default(),
        )
    }

    pub fn default_usd_oracle_rate() -> OracleRate {
        OracleRate::new(IFixedPoint::from_num(1.0), IFixedPoint::from_num(0.001))
    }

    pub fn usd_oracle_config() -> OracleConfig {
        OracleConfig::new(
            OracleProvider::Pyth(PythProvider {
                feed_id: USD_FEED_ID,
                program_id: Pubkey(USD_FEED_ID),
            }),
            Default::default(),
        )
    }
}
