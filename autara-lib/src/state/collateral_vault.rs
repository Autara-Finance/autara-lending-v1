use arch_program::pubkey::Pubkey;
use bytemuck::{Pod, Zeroable};

use crate::{
    constant::MAX_EXPONENT_ABS,
    error::{LendingError, LendingResultExt},
    math::safe_math::SafeMath,
    oracle::{oracle_config::OracleConfig, pod_oracle_provider::PodOracleProvider},
    padding::Padding,
};

use super::super::error::LendingResult;

crate::validate_struct!(CollateralVault, 536);

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[cfg_attr(
    feature = "client",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct CollateralVault {
    /// The mint of the collateral token
    mint: Pubkey,
    /// The decimals of the collateral token mint
    mint_decimals: u64,
    /// The token account that holds the collateral tokens
    vault: Pubkey,
    /// The total amount of collateral tokens in atoms
    total_collateral_atoms: u64,
    /// The oracle configuration to manage the price of the collateral token
    oracle_config: OracleConfig,
    pad: Padding<192>,
}

impl CollateralVault {
    pub fn initialize(
        &mut self,
        mint: Pubkey,
        mint_decimals: u64,
        vault: Pubkey,
        oracle_config: OracleConfig,
    ) -> LendingResult {
        if mint_decimals as i64 > MAX_EXPONENT_ABS {
            return Err(LendingError::UnsupportedMintDecimals.into()).with_msg("collateral vault");
        }
        self.mint = mint;
        self.mint_decimals = mint_decimals;
        self.vault = vault;
        self.oracle_config = oracle_config;
        Ok(())
    }

    pub fn mint(&self) -> &Pubkey {
        &self.mint
    }

    pub fn mint_decimals(&self) -> u8 {
        self.mint_decimals as u8
    }

    pub fn vault(&self) -> &Pubkey {
        &self.vault
    }

    pub fn set_oracle_config(&mut self, oracle_config: OracleConfig) {
        self.oracle_config = oracle_config;
    }

    pub fn oracle_provider(&self) -> &PodOracleProvider {
        self.oracle_config.oracle_provider()
    }

    pub fn oracle_config(&self) -> &OracleConfig {
        &self.oracle_config
    }

    pub fn total_collateral_atoms(&self) -> u64 {
        self.total_collateral_atoms
    }

    pub(super) fn deposit_collateral(&mut self, atoms: u64) -> LendingResult {
        self.total_collateral_atoms = self.total_collateral_atoms.safe_add(atoms)?;
        Ok(())
    }

    pub(super) fn withdraw_collateral(&mut self, atoms: u64) -> LendingResult {
        self.total_collateral_atoms = self.total_collateral_atoms.safe_sub(atoms)?;
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {

    use arch_program::pubkey::Pubkey;

    use crate::oracle::oracle_config::tests::btc_oracle_config;

    use super::*;

    #[allow(non_snake_case)]
    pub const fn BTC(amount: f64) -> u64 {
        (amount * 100_000_000.0) as u64
    }

    pub const BTC_DECIMALS: u64 = 8;

    pub fn create_btc_collateral_vault() -> CollateralVault {
        CollateralVault {
            mint: Pubkey::new_unique(),
            mint_decimals: BTC_DECIMALS,
            vault: Pubkey::new_unique(),
            oracle_config: btc_oracle_config(),
            total_collateral_atoms: 0,
            pad: Padding::default(),
        }
    }
}
