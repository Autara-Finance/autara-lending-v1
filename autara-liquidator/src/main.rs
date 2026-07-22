mod config;
mod preflight;
mod scanner;

use std::time::Duration;

use anyhow::{Context, Result};
use arch_sdk::AsyncArchRpcClient;
use autara_client::client::blockhash_cache::BlockhashCache;
use autara_client::client::single_thread_client::AutaraReadClientImpl;
use clap::Parser;

use crate::config::{parse_hex_pubkey, Args, LiquidatorConfig, TokenFilter};
use crate::scanner::scan_liquidatable_positions;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();
    let config_str = std::fs::read_to_string(&args.config).context("failed to read config file")?;
    let config: LiquidatorConfig =
        serde_json::from_str(&config_str).context("failed to parse config file")?;

    let autara_program_id = parse_hex_pubkey(&config.autara_program_id)?;
    let (liquidator_keypair, liquidator_pubkey) = config.load_keypair()?;
    let network = config.bitcoin_network()?;
    tracing::info!(
        ?liquidator_pubkey,
        network = %config.network,
        dry_run = config.dry_run,
        slippage_bps = config.slippage_bps,
        min_lamports = config.min_lamports,
        max_consecutive_failures = config.max_consecutive_failures,
        "Loaded liquidator keypair"
    );
    if config.liquidator_keypair.contains("admin") {
        tracing::warn!(
            path = %config.liquidator_keypair,
            "liquidator_keypair path looks like an admin key — use a dedicated liquidator key in production"
        );
    }

    let token_filter = TokenFilter::from_config(&config.restrict_tokens)?;
    if token_filter.is_active() {
        tracing::info!(
            "Token filter active: restricting to {} token(s)",
            config.restrict_tokens.len()
        );
    }

    tracing::info!(rpc_url = %config.rpc_url, "Starting liquidator");

    let sdk_config = arch_sdk::Config {
        arch_node_url: config.rpc_url.clone(),
        node_endpoint: String::new(),
        node_username: String::new(),
        node_password: String::new(),
        network,
        titan_url: String::new(),
    };
    let arch_client = AsyncArchRpcClient::new(&sdk_config);

    let mut read_client = AutaraReadClientImpl::new(arch_client.clone(), autara_program_id);
    let blockhash_cache = BlockhashCache::new(arch_client.clone(), None).await?;
    let poll_interval = Duration::from_secs(config.poll_interval_secs);

    let mut consecutive_failures: u32 = 0;

    loop {
        match read_client.reload().await {
            Ok(()) => {
                let stats = scan_liquidatable_positions(
                    &read_client,
                    &token_filter,
                    &arch_client,
                    autara_program_id,
                    &liquidator_keypair,
                    liquidator_pubkey,
                    &blockhash_cache,
                    network,
                    config.dry_run,
                    config.slippage_bps,
                    config.min_lamports,
                )
                .await;

                if !config.dry_run {
                    if stats.live_failure > 0 && stats.live_success == 0 {
                        consecutive_failures =
                            consecutive_failures.saturating_add(stats.live_failure as u32);
                    } else if stats.live_success > 0 {
                        consecutive_failures = 0;
                    }

                    if config.max_consecutive_failures > 0
                        && consecutive_failures >= config.max_consecutive_failures
                    {
                        tracing::error!(
                            consecutive_failures,
                            limit = config.max_consecutive_failures,
                            "CIRCUIT BREAKER — halting liquidator (exit 2)"
                        );
                        std::process::exit(2);
                    }
                }
            }
            Err(e) => {
                tracing::error!("Failed to reload state: {e:#}");
                if !config.dry_run {
                    consecutive_failures = consecutive_failures.saturating_add(1);
                    if config.max_consecutive_failures > 0
                        && consecutive_failures >= config.max_consecutive_failures
                    {
                        tracing::error!(
                            consecutive_failures,
                            limit = config.max_consecutive_failures,
                            "CIRCUIT BREAKER after reload failures — halting (exit 2)"
                        );
                        std::process::exit(2);
                    }
                }
            }
        }
        tokio::time::sleep(poll_interval).await;
    }
}
