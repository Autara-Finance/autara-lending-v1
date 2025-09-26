use autara_lib::metrics::client::{BorrowPositionSummary, SupplyPositionSummary};
use prometheus::GaugeVec;

use crate::prometheus::LiquidityType;

pub struct PositionMetrics {
    position_liquidity: GaugeVec,
    position_liquidity_usd: GaugeVec,
    position_ltv: GaugeVec,
}

impl PositionMetrics {
    pub fn new() -> Self {
        Self {
            position_liquidity: prometheus::register_gauge_vec!(
                "autara_position_liquidity",
                "Position liquidity",
                &[
                    "market_address",
                    "position_address",
                    "user_address",
                    "liquidity_type",
                    "asset"
                ]
            )
            .unwrap(),
            position_liquidity_usd: prometheus::register_gauge_vec!(
                "autara_position_liquidity_usd",
                "Position liquidity in USD",
                &[
                    "market_address",
                    "position_address",
                    "user_address",
                    "liquidity_type",
                    "asset"
                ]
            )
            .unwrap(),
            position_ltv: prometheus::register_gauge_vec!(
                "autara_position_ltv",
                "Position loan to value ratio",
                &["market_address", "position_address", "user_address",]
            )
            .unwrap(),
        }
    }

    pub fn set_borrow_position_liquidity(
        &self,
        market: &str,
        position: &str,
        borrow: &BorrowPositionSummary,
    ) {
        self.position_liquidity
            .with_label_values(&[
                market,
                position,
                &borrow.user,
                LiquidityType::Borrow.as_str(),
                &borrow.supply_mint,
            ])
            .set(borrow.borrow);
        self.position_liquidity
            .with_label_values(&[
                market,
                position,
                &borrow.user,
                LiquidityType::Collateral.as_str(),
                &borrow.collateral_mint,
            ])
            .set(borrow.collateral);
        self.position_liquidity_usd
            .with_label_values(&[
                market,
                position,
                &borrow.user,
                LiquidityType::Borrow.as_str(),
                &borrow.supply_mint,
            ])
            .set(borrow.borrow_usd);
        self.position_liquidity_usd
            .with_label_values(&[
                market,
                position,
                &borrow.user,
                LiquidityType::Collateral.as_str(),
                &borrow.collateral_mint,
            ])
            .set(borrow.collateral_usd);
        self.position_ltv
            .with_label_values(&[market, &borrow.user])
            .set(borrow.ltv);
    }

    pub fn set_supply_position_liquidity(
        &self,
        market: &str,
        position: &str,
        lending: &SupplyPositionSummary,
    ) {
        self.position_liquidity
            .with_label_values(&[
                market,
                position,
                &lending.user,
                LiquidityType::Supply.as_str(),
                &lending.supply_mint,
            ])
            .set(lending.supply);
        self.position_liquidity_usd
            .with_label_values(&[
                market,
                position,
                &lending.user,
                LiquidityType::Supply.as_str(),
                &lending.supply_mint,
            ])
            .set(lending.supply_usd);
    }
}
