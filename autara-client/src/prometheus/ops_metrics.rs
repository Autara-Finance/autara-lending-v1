use prometheus::{GaugeVec, IntGaugeVec};

/// Ops / readiness metrics for oracle health, liquidatable inventory, market
/// config snapshots, and optional pusher funding.
pub struct OpsMetrics {
    oracle_stale: IntGaugeVec,
    oracle_relative_confidence: GaugeVec,
    liquidatable_positions: IntGaugeVec,
    market_max_ltv: GaugeVec,
    market_unhealthy_ltv: GaugeVec,
    market_liquidation_bonus: GaugeVec,
    market_fee_bps: IntGaugeVec,
    market_max_utilisation: GaugeVec,
    pusher_balance_lamports: IntGaugeVec,
}

impl OpsMetrics {
    pub fn new() -> Self {
        Self {
            oracle_stale: prometheus::register_int_gauge_vec!(
                "autara_oracle_stale",
                "1 if market oracle failed staleness/confidence validation on last refresh",
                &["market_address", "side"]
            )
            .unwrap(),
            oracle_relative_confidence: prometheus::register_gauge_vec!(
                "autara_oracle_relative_confidence",
                "Oracle relative confidence (confidence / price)",
                &["market_address", "side"]
            )
            .unwrap(),
            liquidatable_positions: prometheus::register_int_gauge_vec!(
                "autara_market_liquidatable_positions",
                "Number of borrow positions at or above unhealthy_ltv",
                &["market_address"]
            )
            .unwrap(),
            market_max_ltv: prometheus::register_gauge_vec!(
                "autara_market_max_ltv",
                "Configured max_ltv (param-change detection)",
                &["market_address"]
            )
            .unwrap(),
            market_unhealthy_ltv: prometheus::register_gauge_vec!(
                "autara_market_unhealthy_ltv",
                "Configured unhealthy_ltv (param-change detection)",
                &["market_address"]
            )
            .unwrap(),
            market_liquidation_bonus: prometheus::register_gauge_vec!(
                "autara_market_liquidation_bonus",
                "Configured liquidation_bonus (param-change detection)",
                &["market_address"]
            )
            .unwrap(),
            market_fee_bps: prometheus::register_int_gauge_vec!(
                "autara_market_fee_bps",
                "Configured lending_market_fee_in_bps (param-change detection)",
                &["market_address"]
            )
            .unwrap(),
            market_max_utilisation: prometheus::register_gauge_vec!(
                "autara_market_max_utilisation",
                "Configured max_utilisation_rate (param-change detection)",
                &["market_address"]
            )
            .unwrap(),
            pusher_balance_lamports: prometheus::register_int_gauge_vec!(
                "autara_pusher_balance_lamports",
                "Lamport balance of the dedicated oracle pusher signer (if configured)",
                &["pusher_pubkey"]
            )
            .unwrap(),
        }
    }

    pub fn set_oracle_stale(&self, market: &str, side: &str, stale: bool) {
        self.oracle_stale
            .with_label_values(&[market, side])
            .set(if stale { 1 } else { 0 });
    }

    pub fn set_oracle_relative_confidence(&self, market: &str, side: &str, value: f64) {
        self.oracle_relative_confidence
            .with_label_values(&[market, side])
            .set(value);
    }

    pub fn set_liquidatable_positions(&self, market: &str, count: i64) {
        self.liquidatable_positions
            .with_label_values(&[market])
            .set(count);
    }

    pub fn set_market_config(
        &self,
        market: &str,
        max_ltv: f64,
        unhealthy_ltv: f64,
        liquidation_bonus: f64,
        fee_bps: i64,
        max_utilisation: f64,
    ) {
        self.market_max_ltv
            .with_label_values(&[market])
            .set(max_ltv);
        self.market_unhealthy_ltv
            .with_label_values(&[market])
            .set(unhealthy_ltv);
        self.market_liquidation_bonus
            .with_label_values(&[market])
            .set(liquidation_bonus);
        self.market_fee_bps
            .with_label_values(&[market])
            .set(fee_bps);
        self.market_max_utilisation
            .with_label_values(&[market])
            .set(max_utilisation);
    }

    pub fn set_pusher_balance(&self, pubkey: &str, lamports: i64) {
        self.pusher_balance_lamports
            .with_label_values(&[pubkey])
            .set(lamports);
    }
}

impl Default for OpsMetrics {
    fn default() -> Self {
        Self::new()
    }
}
