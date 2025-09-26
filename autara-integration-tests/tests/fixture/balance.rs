#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Balance {
    pub collateral: u64,
    pub supply: u64,
}

impl Balance {
    pub fn delta(&self, other: &Balance) -> BalanceDelta {
        BalanceDelta {
            collateral: self.collateral as i64 - other.collateral as i64,
            supply: self.supply as i64 - other.supply as i64,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BalanceDelta {
    pub collateral: i64,
    pub supply: i64,
}
