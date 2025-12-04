use arch_program::pubkey::Pubkey;
use borsh::{BorshDeserialize, BorshSerialize};
use bytemuck::{Pod, Zeroable};

use crate::{
    constant::MAX_EXPONENT_ABS,
    error::{LendingError, LendingResult},
    oracle::{
        oracle_price::OracleRate,
        oracle_provider::{AccountView, OracleLoader, UncheckedOracleRate},
    },
};

#[repr(C)]
#[derive(Debug, PartialEq, Eq, Copy, Clone, Pod, Zeroable, BorshSerialize, BorshDeserialize)]
#[cfg_attr(
    feature = "client",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct PythProvider {
    pub feed_id: [u8; 32],
    pub program_id: Pubkey,
}

impl OracleLoader for PythProvider {
    fn load_oracle_price<D: std::ops::Deref<Target = [u8]>>(
        &self,
        view: AccountView<D>,
    ) -> LendingResult<UncheckedOracleRate> {
        if *view.owner != self.program_id {
            return Err(LendingError::InvalidPythOracleAccount.into());
        }
        let pyth_price = bytemuck::try_from_bytes::<PythPriceAccount>(&view.data)
            .map_err(|_| LendingError::InvalidPythOracleAccount)?;
        if pyth_price.pyth_price.id != self.feed_id {
            return Err(LendingError::InvalidOracleFeedId.into());
        }
        if pyth_price.pyth_price.price.expo > MAX_EXPONENT_ABS
            || pyth_price.pyth_price.price.expo < -MAX_EXPONENT_ABS
        {
            return Err(LendingError::InvalidPythOracleAccount.into());
        }
        let expo = pyth_price.pyth_price.price.expo as i8;
        Ok(UncheckedOracleRate::new(
            OracleRate::try_from_price_expo_conf(
                pyth_price.pyth_price.price.price,
                pyth_price.pyth_price.price.conf,
                expo,
            )?,
            pyth_price.pyth_price.price.publish_time,
        ))
    }
}

#[derive(Debug, Pod, Zeroable, Clone, Copy)]
#[repr(C)]
pub struct PythPriceAccount {
    pub pyth_price: PythPrice,
}

#[derive(Debug, Pod, Zeroable, Clone, Copy)]
#[repr(C)]
pub struct PythPrice {
    pub id: [u8; 32],
    pub price: PriceData,
    pub ema_price: PriceData,
    pub metadata: Metadata,
}

impl PythPrice {
    #[cfg(feature = "client")]
    pub fn from_dummy(id: [u8; 32], price: f64) -> Self {
        use std::time::SystemTime;
        let expo = -8;
        let price_u64 = (price * 10f64.powi(-expo)).round() as u64;
        let conf_u64 = price_u64 / 100;
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        Self {
            id,
            price: PriceData {
                price: price_u64,
                conf: conf_u64,
                expo: expo as i64,
                publish_time: now,
            },
            ema_price: PriceData {
                price: price_u64,
                conf: conf_u64,
                expo: expo as i64,
                publish_time: now,
            },
            metadata: Metadata {
                slot: 0,
                proof_available_time: now,
                prev_publish_time: now - 10,
            },
        }
    }
}

#[derive(Debug, Pod, Zeroable, Clone, Copy)]
#[repr(C)]
pub struct PriceData {
    pub price: u64,
    pub conf: u64,
    pub expo: i64,
    pub publish_time: i64,
}

#[derive(Debug, Pod, Zeroable, Clone, Copy)]
#[repr(C)]
pub struct Metadata {
    pub slot: u64,
    pub proof_available_time: i64,
    pub prev_publish_time: i64,
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::math::ifixed_point::IFixedPoint;

    fn create_test_pubkey() -> Pubkey {
        Pubkey::from([1u8; 32])
    }

    fn create_test_feed_id() -> [u8; 32] {
        [2u8; 32]
    }

    fn create_pyth_provider() -> PythProvider {
        PythProvider {
            feed_id: create_test_feed_id(),
            program_id: create_test_pubkey(),
        }
    }

    fn create_pyth_price_account(feed_id: [u8; 32], price: u64, conf: u64, expo: i64) -> Vec<u8> {
        let pyth_price = PythPrice {
            id: feed_id,
            price: PriceData {
                price,
                conf,
                expo,
                publish_time: 1234567890,
            },
            ema_price: PriceData {
                price: 0,
                conf: 0,
                expo: 0,
                publish_time: 0,
            },
            metadata: Metadata {
                slot: 100,
                proof_available_time: 1234567890,
                prev_publish_time: 1234567889,
            },
        };

        let account = PythPriceAccount { pyth_price };
        let bytes = bytemuck::bytes_of(&account);
        bytes.to_vec()
    }

    #[test]
    fn test_load_oracle_price_success() {
        let provider = create_pyth_provider();
        let key = create_test_pubkey();
        let owner = create_test_pubkey();

        // Create valid price data with expo = -8 (8 decimal places)
        let price_data = create_pyth_price_account(
            create_test_feed_id(),
            10000000000u64, // 100.00000000 with 8 decimals
            5000000u64,     // 0.05000000 confidence with 8 decimals
            -8,
        );

        let result = provider.load_oracle_price((&key, price_data, &owner).into());
        assert!(result.is_ok());

        let oracle_rate = result.unwrap().unsafe_rate();
        // Price should be 100.0
        assert_eq!(oracle_rate.rate(), IFixedPoint::from(100));
        // Confidence should be 0.05
        assert_eq!(
            oracle_rate.confidence(),
            IFixedPoint::from_i64_u64_ratio(5, 100)
        );
    }

    #[test]
    fn test_load_oracle_price_invalid_owner() {
        let provider = create_pyth_provider();
        let key = create_test_pubkey();
        let wrong_owner = Pubkey::from([99u8; 32]); // Different from provider.program_id

        let price_data =
            create_pyth_price_account(create_test_feed_id(), 10000000000u64, 5000000u64, -8);

        let result = provider.load_oracle_price((&key, price_data, &wrong_owner).into());
        assert!(result.is_err());
        assert_eq!(*result.unwrap_err(), LendingError::InvalidPythOracleAccount);
    }

    #[test]
    fn test_load_oracle_price_invalid_feed_id() {
        let provider = create_pyth_provider();
        let key = create_test_pubkey();
        let owner = create_test_pubkey();

        // Create price data with wrong feed ID
        let wrong_feed_id = [99u8; 32];
        let price_data = create_pyth_price_account(wrong_feed_id, 10000000000u64, 5000000u64, -8);

        let result = provider.load_oracle_price((&key, price_data, &owner).into());
        assert!(result.is_err());
        assert_eq!(*result.unwrap_err(), LendingError::InvalidOracleFeedId);
    }

    #[test]
    fn test_load_oracle_price_expo_too_negative() {
        let provider = create_pyth_provider();
        let key = create_test_pubkey();
        let owner = create_test_pubkey();

        // Create price data with exponent more negative than MAX_EXPONENT
        let price_data = create_pyth_price_account(
            create_test_feed_id(),
            10000000000u64,
            5000000u64,
            -(MAX_EXPONENT_ABS + 1), // Too negative
        );

        let result = provider.load_oracle_price((&key, price_data, &owner).into());
        assert!(result.is_err());
        assert_eq!(*result.unwrap_err(), LendingError::InvalidPythOracleAccount);
    }

    #[test]
    fn test_load_oracle_price_max_valid_expo() {
        let provider = create_pyth_provider();
        let key = create_test_pubkey();
        let owner = create_test_pubkey();

        let price_data = create_pyth_price_account(
            create_test_feed_id(),
            1000000000000000000u64,
            50000000000000000u64,
            -MAX_EXPONENT_ABS,
        );

        let result = provider.load_oracle_price((&key, price_data, &owner).into());
        assert!(result.is_ok());
    }

    #[test]
    fn test_load_oracle_price_zero_expo() {
        let provider = create_pyth_provider();
        let key = create_test_pubkey();
        let owner = create_test_pubkey();

        let price_data = create_pyth_price_account(create_test_feed_id(), 100u64, 5u64, 0);

        let result = provider.load_oracle_price((&key, price_data, &owner).into());
        assert!(result.is_ok());

        let oracle_rate = result.unwrap().unsafe_rate();
        assert_eq!(oracle_rate.rate(), IFixedPoint::from(100));
        assert_eq!(oracle_rate.confidence(), IFixedPoint::from(5));
    }

    #[test]
    fn test_load_oracle_price_invalid_data_format() {
        let provider = create_pyth_provider();
        let key = create_test_pubkey();
        let owner = create_test_pubkey();

        // Create invalid data (too short)
        let invalid_data = vec![1u8; 10]; // Much smaller than PythPriceAccount

        let result = provider.load_oracle_price((&key, invalid_data, &owner).into());
        assert!(result.is_err());
        assert_eq!(*result.unwrap_err(), LendingError::InvalidPythOracleAccount);
    }

    #[test]
    fn test_load_oracle_price_different_decimal_places() {
        let provider = create_pyth_provider();
        let key = create_test_pubkey();
        let owner = create_test_pubkey();

        let test_cases = vec![
            (
                -1,
                1000u64,
                10u64,
                IFixedPoint::from(100),
                IFixedPoint::from(1),
            ),
            (
                -2,
                10000u64,
                100u64,
                IFixedPoint::from(100),
                IFixedPoint::from(1),
            ),
            (
                -6,
                100000000u64,
                1000000u64,
                IFixedPoint::from(100),
                IFixedPoint::from(1),
            ),
        ];

        for (expo, price, conf, expected_rate, expected_conf) in test_cases {
            let price_data = create_pyth_price_account(create_test_feed_id(), price, conf, expo);

            let result = provider.load_oracle_price((&key, price_data, &owner).into());
            assert!(result.is_ok(), "Failed for expo {}", expo);

            let oracle_rate = result.unwrap().unsafe_rate();
            assert_eq!(
                oracle_rate.rate(),
                expected_rate,
                "Rate mismatch for expo {}",
                expo
            );
            assert_eq!(
                oracle_rate.confidence(),
                expected_conf,
                "Confidence mismatch for expo {}",
                expo
            );
        }
    }
}
