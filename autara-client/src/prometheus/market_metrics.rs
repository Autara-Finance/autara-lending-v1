use autara_lib::metrics::client::LiquiditySummary;
use prometheus::GaugeVec;

use crate::prometheus::LiquidityType;

pub struct MarketMetrics {
    market_utilization_rate: GaugeVec,
    market_liquidity: GaugeVec,
    market_liquidity_usd: GaugeVec,
    market_borrow_rate: GaugeVec,
    market_lending_rate: GaugeVec,
}

impl MarketMetrics {
    pub fn new() -> Self {
        Self {
            market_utilization_rate: prometheus::register_gauge_vec!(
                "autara_market_utilization_rate",
                "Market utilization rate",
                &["market_address"]
            )
            .unwrap(),
            market_liquidity: prometheus::register_gauge_vec!(
                "autara_market_liquidity",
                "Market liquidity",
                &["market_address", "liquidity_type", "asset"]
            )
            .unwrap(),
            market_liquidity_usd: prometheus::register_gauge_vec!(
                "autara_market_liquidity_usd",
                "Market liquidity",
                &["market_address", "liquidity_type", "asset"]
            )
            .unwrap(),
            market_borrow_rate: prometheus::register_gauge_vec!(
                "autara_market_borrow_rate",
                "Market borrow rate APY",
                &["market_address"]
            )
            .unwrap(),
            market_lending_rate: prometheus::register_gauge_vec!(
                "autara_market_lending_rate",
                "Market lending rate APY",
                &["market_address"]
            )
            .unwrap(),
        }
    }

    pub fn set_market_utilization_rate(&self, market: &str, rate: f64) {
        self.market_utilization_rate
            .with_label_values(&[market])
            .set(rate);
    }

    pub fn set_market_liquidity(&self, market: &str, liquidity_summary: &LiquiditySummary) {
        self.market_liquidity
            .with_label_values(&[
                market,
                LiquidityType::Supply.as_str(),
                &liquidity_summary.supply_mint,
            ])
            .set(liquidity_summary.supply);
        self.market_liquidity
            .with_label_values(&[
                market,
                LiquidityType::Borrow.as_str(),
                &liquidity_summary.supply_mint,
            ])
            .set(liquidity_summary.borrow);
        self.market_liquidity
            .with_label_values(&[
                market,
                LiquidityType::Collateral.as_str(),
                &liquidity_summary.collateral_mint,
            ])
            .set(liquidity_summary.collateral);
        self.market_liquidity_usd
            .with_label_values(&[
                market,
                LiquidityType::Supply.as_str(),
                &liquidity_summary.supply_mint,
            ])
            .set(liquidity_summary.supply_usd);
        self.market_liquidity_usd
            .with_label_values(&[
                market,
                LiquidityType::Borrow.as_str(),
                &liquidity_summary.supply_mint,
            ])
            .set(liquidity_summary.borrow_usd);
        self.market_liquidity_usd
            .with_label_values(&[
                market,
                LiquidityType::Collateral.as_str(),
                &liquidity_summary.collateral_mint,
            ])
            .set(liquidity_summary.collateral_usd);
    }

    pub fn set_market_borrow_and_lending_rate(
        &self,
        market: &str,
        borrow_rate: f64,
        lending_rate: f64,
    ) {
        self.market_borrow_rate
            .with_label_values(&[market])
            .set(borrow_rate);
        self.market_lending_rate
            .with_label_values(&[market])
            .set(lending_rate);
    }
}
