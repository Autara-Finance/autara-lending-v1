#[cfg(feature = "client")]
pub mod client {
    use std::ops::Deref;

    use crate::{
        error::LendingResult,
        state::{
            borrow_position::BorrowPosition, market::Market, market_wrapper::MarketWrapper,
            supply_position::SupplyPosition,
        },
    };

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct LiquiditySummary {
        pub supply_mint: String,
        pub collateral_mint: String,
        pub supply: f64,
        pub borrow: f64,
        pub collateral: f64,
        pub supply_usd: f64,
        pub borrow_usd: f64,
        pub collateral_usd: f64,
    }

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct BorrowPositionSummary {
        pub user: String,
        pub supply_mint: String,
        pub collateral_mint: String,
        pub borrow: f64,
        pub borrow_usd: f64,
        pub collateral: f64,
        pub collateral_usd: f64,
        pub ltv: f64,
    }

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct SupplyPositionSummary {
        pub user: String,
        pub supply_mint: String,
        pub supply: f64,
        pub supply_usd: f64,
    }

    impl<M: Deref<Target = Market>> MarketWrapper<M> {
        pub fn liquidity_summary(&self) -> LendingResult<LiquiditySummary> {
            let supply_vault_summary = self.market().supply_vault().get_summary()?;
            let total_collateral_atoms = self.market().collateral_vault().total_collateral_atoms();
            let supply = supply_vault_summary.total_supply as f64
                / 10f64.powi(self.market().supply_vault().mint_decimals() as i32);
            let borrow = supply_vault_summary.total_borrow as f64
                / 10f64.powi(self.market().supply_vault().mint_decimals() as i32);
            let collateral = total_collateral_atoms as f64
                / 10f64.powi(self.market().collateral_vault().mint_decimals() as i32);
            let supply_price = self.supply_oracle().rate().to_float();
            let supply_usd = supply * supply_price;
            let borrow_usd = borrow * supply_price;
            let collateral_usd = collateral * self.collateral_oracle().rate().to_float();
            Ok(LiquiditySummary {
                supply_mint: self.market().supply_vault().mint().to_string(),
                collateral_mint: self.market().collateral_vault().mint().to_string(),
                supply,
                borrow,
                collateral,
                supply_usd,
                borrow_usd,
                collateral_usd,
            })
        }

        pub fn supply_position_summary(
            &self,
            position: &SupplyPosition,
        ) -> LendingResult<SupplyPositionSummary> {
            let total_supply = self.market().supply_position_info(position)?;
            Ok(SupplyPositionSummary {
                user: position.authority().to_string(),
                supply_mint: self.market().supply_vault().mint().to_string(),
                supply: total_supply as f64
                    / 10f64.powi(self.market().supply_vault().mint_decimals() as i32),
                supply_usd: (total_supply as f64
                    / 10f64.powi(self.market().supply_vault().mint_decimals() as i32))
                    * self.supply_oracle().rate().to_float(),
            })
        }

        pub fn borrow_position_summary(
            &self,
            position: &BorrowPosition,
        ) -> LendingResult<BorrowPositionSummary> {
            let health = self.borrow_position_health(position)?;
            Ok(BorrowPositionSummary {
                user: position.authority().to_string(),
                supply_mint: self.market().supply_vault().mint().to_string(),
                collateral_mint: self.market().collateral_vault().mint().to_string(),
                borrow: health.borrowed_atoms as f64
                    / 10f64.powi(self.market().supply_vault().mint_decimals() as i32),
                borrow_usd: (health.borrowed_atoms as f64
                    / 10f64.powi(self.market().supply_vault().mint_decimals() as i32))
                    * self.supply_oracle().rate().to_float(),
                collateral: position.collateral_deposited_atoms() as f64
                    / 10f64.powi(self.market().collateral_vault().mint_decimals() as i32),
                collateral_usd: (position.collateral_deposited_atoms() as f64
                    / 10f64.powi(self.market().collateral_vault().mint_decimals() as i32))
                    * self.collateral_oracle().rate().to_float(),
                ltv: health.ltv.to_float(),
            })
        }
    }
}
