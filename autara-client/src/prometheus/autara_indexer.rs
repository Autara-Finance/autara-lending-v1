use std::{sync::Arc, time::Duration};

use tokio::time::Instant;

use crate::{
    client::{read::AutaraReadClient, shared_autara_state::AutaraSharedState},
    prometheus::{market_metrics::MarketMetrics, position_metrics::PositionMetrics},
};

pub struct PrometheusAutaraIndexer {
    state: Arc<AutaraSharedState>,
    market_metrics: MarketMetrics,
    position_metrics: PositionMetrics,
    refresh_interval: Duration,
}

impl PrometheusAutaraIndexer {
    pub fn new(state: Arc<AutaraSharedState>, refresh_interval: Duration) -> Self {
        Self {
            state,
            market_metrics: MarketMetrics::new(),
            position_metrics: PositionMetrics::new(),
            refresh_interval,
        }
    }

    pub fn spawn(self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                let now = Instant::now();
                self.refresh();
                let elapsed = now.elapsed();
                tracing::info!("PrometheusAutaraIndexer refreshed metrics in {:?}", elapsed);
                tokio::time::sleep(self.refresh_interval).await;
            }
        })
    }

    fn refresh(&self) {
        for (market_address, market) in self.state.all_markets() {
            let market_address = market_address.to_string();
            if let Ok(liquidity_summary) = market.liquidity_summary() {
                self.market_metrics
                    .set_market_liquidity(&market_address, &liquidity_summary);
            }
            if let Ok(utilisation_rate) = market.market().supply_vault().utilisation_rate() {
                self.market_metrics
                    .set_market_utilization_rate(&market_address, utilisation_rate.to_float());
                let borrow_rate = market.market().supply_vault().last_borrow_interest_rate();
                let lending_rate = borrow_rate
                    .adjust_for_utilisation_rate(utilisation_rate)
                    .and_then(|x| x.approximate_apy());
                if let (Ok(borrow_rate), Ok(lending_rate)) =
                    (borrow_rate.approximate_apy(), lending_rate)
                {
                    self.market_metrics.set_market_borrow_and_lending_rate(
                        &market_address,
                        borrow_rate,
                        lending_rate,
                    );
                }
            }
        }
        for (position_address, position) in self.state.all_borrow_position() {
            let Some(market) = self.state.get_market(position.market()) else {
                continue;
            };
            if let Ok(borrow_summary) = market.borrow_position_summary(&position) {
                let position_address = position_address.to_string();
                let market_address = position.market().to_string();
                self.position_metrics.set_borrow_position_liquidity(
                    &market_address,
                    &position_address,
                    &borrow_summary,
                );
            }
        }
        for (position_address, position) in self.state.all_supply_position() {
            let Some(market) = self.state.get_market(position.market()) else {
                continue;
            };
            if let Ok(lending_summary) = market.supply_position_summary(&position) {
                let position_address = position_address.to_string();
                let market_address = position.market().to_string();
                self.position_metrics.set_supply_position_liquidity(
                    &market_address,
                    &position_address,
                    &lending_summary,
                );
            }
        }
    }
}
