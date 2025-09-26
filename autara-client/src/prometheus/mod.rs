pub mod autara_indexer;
pub mod exporter;
pub mod market_metrics;
pub mod position_metrics;

#[derive(Debug, Clone, Copy)]
pub enum LiquidityType {
    Supply,
    Borrow,
    Collateral,
}

impl LiquidityType {
    pub const fn as_str(&self) -> &'static str {
        match self {
            LiquidityType::Supply => "supply",
            LiquidityType::Borrow => "borrow",
            LiquidityType::Collateral => "collateral",
        }
    }
}
