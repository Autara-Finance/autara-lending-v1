use std::{collections::HashMap, sync::Arc, time::Duration};

use arch_sdk::{arch_program::pubkey::Pubkey, AsyncArchRpcClient};
use tokio::time::Instant;

use crate::{
    client::{
        read::AutaraReadClient, shared_autara_state::AutaraSharedState,
        single_thread_client::get_unix_timestamp,
    },
    prometheus::{
        market_metrics::MarketMetrics, ops_metrics::OpsMetrics, position_metrics::PositionMetrics,
    },
    rpc_ext::ArchAsyncRpcExt,
};

struct VaultAccounting {
    market_address: String,
    vault_type: &'static str,
    vault_address: Pubkey,
    accounted_atoms: u64,
}

pub struct PrometheusAutaraIndexer {
    state: Arc<AutaraSharedState>,
    rpc_client: AsyncArchRpcClient,
    pusher_pubkey: Option<Pubkey>,
    market_metrics: MarketMetrics,
    position_metrics: PositionMetrics,
    ops_metrics: OpsMetrics,
    refresh_interval: Duration,
}

impl PrometheusAutaraIndexer {
    pub fn new(
        state: Arc<AutaraSharedState>,
        rpc_client: AsyncArchRpcClient,
        pusher_pubkey: Option<Pubkey>,
        refresh_interval: Duration,
    ) -> Self {
        Self {
            state,
            rpc_client,
            pusher_pubkey,
            market_metrics: MarketMetrics::new(),
            position_metrics: PositionMetrics::new(),
            ops_metrics: OpsMetrics::new(),
            refresh_interval,
        }
    }

    pub fn spawn(self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                let now = Instant::now();
                self.refresh().await;
                let elapsed = now.elapsed();
                tracing::info!("PrometheusAutaraIndexer refreshed metrics in {:?}", elapsed);
                tokio::time::sleep(self.refresh_interval).await;
            }
        })
    }

    async fn refresh(&self) {
        if let Some(pusher_pubkey) = self.pusher_pubkey {
            match self.rpc_client.read_account_info(pusher_pubkey).await {
                Ok(account) => self
                    .ops_metrics
                    .set_pusher_balance(&pusher_pubkey.to_string(), account.lamports as i64),
                Err(error) => tracing::warn!(
                    ?error,
                    ?pusher_pubkey,
                    "Failed to read dedicated pusher signer balance"
                ),
            }
        }

        let mut liquidatable: HashMap<String, i64> = HashMap::new();
        let mut vaults = Vec::new();
        let unix_timestamp = get_unix_timestamp();

        for (market_address, market, stale) in self.state.all_markets_maybe_stale() {
            let market_address = market_address.to_string();
            self.ops_metrics
                .set_oracle_stale(&market_address, "supply", stale);
            self.ops_metrics
                .set_oracle_stale(&market_address, "collateral", stale);
            self.ops_metrics.set_oracle_publish_time_age(
                &market_address,
                "supply",
                unix_timestamp - market.supply_oracle_publish_time(),
            );
            self.ops_metrics.set_oracle_publish_time_age(
                &market_address,
                "collateral",
                unix_timestamp - market.collateral_oracle_publish_time(),
            );

            if let Ok(rel) = market.supply_oracle().relative_confidence() {
                self.ops_metrics.set_oracle_relative_confidence(
                    &market_address,
                    "supply",
                    rel.to_float(),
                );
            }
            if let Ok(rel) = market.collateral_oracle().relative_confidence() {
                self.ops_metrics.set_oracle_relative_confidence(
                    &market_address,
                    "collateral",
                    rel.to_float(),
                );
            }

            let cfg = market.market().config();
            let ltv = cfg.ltv_config();
            self.ops_metrics.set_market_config(
                &market_address,
                ltv.max_ltv.to_float(),
                ltv.unhealthy_ltv.to_float(),
                ltv.liquidation_bonus.to_float(),
                cfg.lending_market_fee_in_bps() as i64,
                cfg.max_utilisation_rate().to_float(),
            );
            liquidatable.entry(market_address.clone()).or_insert(0);

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

            let supply_vault = market.market().supply_vault();
            match (supply_vault.total_supply(), supply_vault.total_borrow()) {
                (Ok(total_supply), Ok(total_borrow)) if total_supply >= total_borrow => {
                    let accounted_atoms = total_supply - total_borrow;
                    vaults.push(VaultAccounting {
                        market_address: market_address.clone(),
                        vault_type: "supply",
                        vault_address: *supply_vault.vault(),
                        accounted_atoms,
                    });
                }
                _ => {
                    tracing::warn!(
                        market = %market_address,
                        "Unable to derive supply vault accounting for reconciliation"
                    );
                    self.ops_metrics
                        .set_vault_reconciliation_failed(&market_address, "supply");
                }
            }

            let collateral_vault = market.market().collateral_vault();
            vaults.push(VaultAccounting {
                market_address: market_address.clone(),
                vault_type: "collateral",
                vault_address: *collateral_vault.vault(),
                accounted_atoms: collateral_vault.total_collateral_atoms(),
            });
        }

        self.reconcile_vaults(&vaults).await;

        for (position_address, position) in self.state.all_borrow_position() {
            let Some(market) = self.state.get_market(position.market()) else {
                continue;
            };
            let market_address = position.market().to_string();
            let unhealthy = market
                .market()
                .config()
                .ltv_config()
                .unhealthy_ltv
                .to_float();
            if let Ok(borrow_summary) = market.borrow_position_summary(&position) {
                let position_address = position_address.to_string();
                if borrow_summary.ltv >= unhealthy {
                    *liquidatable.entry(market_address.clone()).or_insert(0) += 1;
                }
                self.position_metrics.set_borrow_position_liquidity(
                    &market_address,
                    &position_address,
                    &borrow_summary,
                );
            }
        }

        for (market, count) in liquidatable {
            self.ops_metrics.set_liquidatable_positions(&market, count);
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

    async fn reconcile_vaults(&self, vaults: &[VaultAccounting]) {
        if vaults.is_empty() {
            return;
        }

        let vault_addresses = vaults
            .iter()
            .map(|vault| vault.vault_address)
            .collect::<Vec<_>>();
        let balances = match self
            .rpc_client
            .get_multiple_accounts_batch(&vault_addresses)
            .await
        {
            Ok(accounts) => accounts
                .iter()
                .filter_map(|account| {
                    token_account_balance(&account.data).map(|amount| (account.key, amount))
                })
                .collect::<HashMap<_, _>>(),
            Err(error) => {
                tracing::warn!(?error, "Failed to collect market vault balances");
                for vault in vaults {
                    self.ops_metrics
                        .set_vault_reconciliation_failed(&vault.market_address, vault.vault_type);
                }
                return;
            }
        };

        for vault in vaults {
            if let Some(actual_atoms) = balances.get(&vault.vault_address) {
                self.ops_metrics.set_vault_reconciliation(
                    &vault.market_address,
                    vault.vault_type,
                    *actual_atoms,
                    vault.accounted_atoms,
                );
            } else {
                tracing::warn!(
                    market = %vault.market_address,
                    vault_type = vault.vault_type,
                    vault = ?vault.vault_address,
                    "Unable to decode market vault token balance"
                );
                self.ops_metrics
                    .set_vault_reconciliation_failed(&vault.market_address, vault.vault_type);
            }
        }
    }
}

fn token_account_balance(data: &[u8]) -> Option<u64> {
    data.get(64..72)
        .and_then(|bytes| bytes.try_into().ok())
        .map(u64::from_le_bytes)
}

#[cfg(test)]
mod tests {
    use super::token_account_balance;

    #[test]
    fn reads_token_account_balance_from_standard_amount_offset() {
        let mut data = vec![0; 72];
        data[64..72].copy_from_slice(&42_u64.to_le_bytes());

        assert_eq!(token_account_balance(&data), Some(42));
    }

    #[test]
    fn rejects_account_data_that_cannot_contain_a_balance() {
        assert_eq!(token_account_balance(&[0; 71]), None);
    }
}
