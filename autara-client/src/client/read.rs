use std::ops::Deref;

use anyhow::Context;
use arch_sdk::arch_program::pubkey::Pubkey;
use autara_lib::state::{
    borrow_position::{BorrowPosition, BorrowPositionHealth},
    global_config::GlobalConfig,
    market::Market,
    market_wrapper::MarketWrapper,
    supply_position::SupplyPosition,
};
use serde::{Deserialize, Serialize};

#[auto_impl::auto_impl(&, Arc, Box)]
pub trait AutaraReadClient: Send + Sync {
    fn autara_program_id(&self) -> &Pubkey;
    fn all_markets(
        &self,
    ) -> impl Iterator<Item = (Pubkey, MarketWrapper<impl Deref<Target = Market>>)>;
    fn all_borrow_position(
        &self,
    ) -> impl Iterator<Item = (Pubkey, impl Deref<Target = BorrowPosition>)>;
    fn all_supply_position(
        &self,
    ) -> impl Iterator<Item = (Pubkey, impl Deref<Target = SupplyPosition>)>;
    fn get_market(&self, market: &Pubkey) -> Option<MarketWrapper<impl Deref<Target = Market>>>;
    fn get_borrow_position(
        &self,
        market: &Pubkey,
        authority: &Pubkey,
    ) -> (Pubkey, Option<impl Deref<Target = BorrowPosition>>);
    fn get_supply_position(
        &self,
        market: &Pubkey,
        authority: &Pubkey,
    ) -> (Pubkey, Option<impl Deref<Target = SupplyPosition>>);
    fn get_global_config(&self) -> Option<impl Deref<Target = GlobalConfig>>;
    fn get_borrow_position_health(
        &self,
        market: &Pubkey,
        authority: &Pubkey,
    ) -> anyhow::Result<BorrowPositionHealth> {
        let borrow_position = self
            .get_borrow_position(&market, authority)
            .1
            .context("borrow position not found")?;
        let market_w = self.get_market(&market).context("market not found")?;
        Ok(market_w.borrow_position_health(&borrow_position)?)
    }
    fn user_positions(&self, authority: &Pubkey) -> UserPositions {
        let mut supply_positions = Vec::new();
        let mut borrow_positions = Vec::new();
        for (market_key, market) in self.all_markets() {
            let (_, supply_position) = self.get_supply_position(&market_key, authority);
            if let Some(supply_position) = supply_position {
                supply_positions.push(SupplyPositionInfo {
                    owned_atoms: market
                        .market()
                        .supply_position_info(&supply_position)
                        .unwrap_or_default(),
                    supply_position: *supply_position,
                });
            }
            let (_, borrow_position) = self.get_borrow_position(&market_key, authority);
            if let Some(borrow_position) = borrow_position {
                borrow_positions.push(BorrowPositionInfo {
                    health: market
                        .borrow_position_health(&borrow_position)
                        .unwrap_or_default(),
                    borrow_position: *borrow_position,
                });
            }
        }
        UserPositions {
            supply_positions,
            borrow_positions,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserPositions {
    pub supply_positions: Vec<SupplyPositionInfo>,
    pub borrow_positions: Vec<BorrowPositionInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BorrowPositionInfo {
    pub borrow_position: BorrowPosition,
    pub health: BorrowPositionHealth,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupplyPositionInfo {
    pub supply_position: SupplyPosition,
    pub owned_atoms: u64,
}
