use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;

use anyhow::{anyhow, Context};
use arch_sdk::arch_program::bitcoin::key::Keypair;
use arch_sdk::arch_program::bitcoin::Network;
use arch_sdk::arch_program::pubkey::Pubkey;
use arch_sdk::with_secret_key_file;
use autara_client::api::server::build_autara_server;
use autara_client::client::client_with_signer::AutaraFullClientWithSigner;
use autara_client::client::read::AutaraReadClient;
use autara_client::client::shared_autara_state::AutaraSharedState;
use autara_client::config::{autara_oracle_stage_program_id, autara_stage_program_id, ArchConfig};
use autara_client::prometheus::autara_indexer::PrometheusAutaraIndexer;
use autara_client::prometheus::exporter::PrometheusExporter;
use autara_client::test::TokenMinter;
use autara_lib::interest_rate::interest_rate_kind::InterestRateCurveKind;
use autara_lib::ixs::CreateMarketInstruction;
use autara_lib::math::ifixed_point::IFixedPoint;
use autara_lib::oracle::oracle_config::OracleConfig;
use autara_lib::pda::find_market_pda;
use autara_lib::state::market_config::LtvConfig;
use autara_pyth::{fetch_and_push_feeds, BTC_FEED, ETH_FEED, USDC_FEED, USDT_FEED};
use clap::Parser;
use jsonrpsee::server::{RpcServiceBuilder, Server};
use serde::Deserialize;
use tower_http::compression::CompressionLayer;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "autara-server")]
#[command(about = "Autara Lending API server")]
struct Args {
    /// Path to tokens.json config file (produced by `autara-cli token setup`)
    #[arg(long)]
    tokens: String,

    /// Autara program ID (hex pubkey). Defaults to keys/autara-stage.key address.
    #[arg(long)]
    program_id: Option<String>,

    /// Oracle program ID (hex pubkey). Defaults to keys/autara-pyth-stage.key address.
    #[arg(long)]
    oracle_program_id: Option<String>,

    /// Path to signer key file (authority for market creation, minting, etc.).
    /// Falls back to AUTARA_SIGNER_KEY env var if not provided.
    #[arg(long)]
    signer: Option<String>,

    /// Arch node URL
    #[arg(long, default_value = "https://rpc.testnet.arch.network")]
    arch_node: String,

    /// Bitcoin network (regtest, testnet, mainnet)
    #[arg(long, default_value = "testnet")]
    network: String,

    /// RPC server listen address
    #[arg(long, default_value = "0.0.0.0:62776")]
    listen: String,

    /// Prometheus exporter listen address
    #[arg(long, default_value = "0.0.0.0:62777")]
    prometheus: String,

    /// Path to markets deploy-plan JSON (list of markets, each with curator + LTV config).
    /// If not provided, no markets are created and the server only runs the API.
    #[arg(long, default_value = "markets.json")]
    markets: String,
}

#[derive(Deserialize)]
struct TokensFile {
    tokens: HashMap<String, TokenEntry>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct TokenEntry {
    mint: String,
    decimals: u8,
    #[serde(rename = "keyFile")]
    key_file: String,
    #[serde(rename = "mintAuthorityKeyFile")]
    mint_authority_key_file: String,
    #[serde(rename = "mintAuthority", default)]
    mint_authority: Option<String>,
}

#[derive(Deserialize)]
struct MarketsFile {
    markets: Vec<MarketEntry>,
}

#[derive(Deserialize)]
struct MarketEntry {
    /// Optional human-readable label (used only in logs).
    #[serde(default)]
    name: Option<String>,
    /// Token name from tokens.json (e.g. "BTC").
    supply: String,
    /// Token name from tokens.json (e.g. "USDC").
    collateral: String,
    /// Path to the curator keypair file. Auto-generated on first run if missing.
    #[serde(rename = "curatorKeyFile")]
    curator_key_file: String,
    /// Market index — set to a different value to create multiple markets with the
    /// same (curator, supply, collateral) tuple. Defaults to 0.
    #[serde(default)]
    index: u8,
    #[serde(rename = "ltvConfig")]
    ltv_config: LtvConfig,
    #[serde(default, rename = "maxUtilisationRate")]
    max_utilisation_rate: Option<IFixedPoint>,
    #[serde(default, rename = "lendingMarketFeeInBps")]
    lending_market_fee_in_bps: Option<u16>,
    #[serde(default, rename = "interestRate")]
    interest_rate: Option<InterestRateCurveKind>,
}

fn parse_network(network: &str) -> anyhow::Result<Network> {
    match network.to_lowercase().as_str() {
        "regtest" => Ok(Network::Regtest),
        "testnet" => Ok(Network::Testnet),
        "mainnet" | "bitcoin" => Ok(Network::Bitcoin),
        _ => anyhow::bail!("Invalid network: {network}. Use regtest, testnet, or mainnet"),
    }
}

fn parse_pubkey(s: &str) -> anyhow::Result<Pubkey> {
    if s.len() == 64 {
        let bytes: [u8; 32] = hex::decode(s)
            .context("Invalid hex pubkey")?
            .try_into()
            .map_err(|_| anyhow!("Invalid pubkey length"))?;
        return Ok(Pubkey::from(bytes));
    }
    std::str::FromStr::from_str(s).map_err(|e| anyhow!("Invalid pubkey format: {}", e))
}

/// Map token name -> Pyth feed ID (bytes)
fn pyth_feed_for_token(name: &str) -> Option<[u8; 32]> {
    let feed_hex = match name.to_uppercase().as_str() {
        "BTC" => BTC_FEED,
        "USDC" => USDC_FEED,
        "ETH" => ETH_FEED,
        "USDT" => USDT_FEED,
        // aUSD is a USD-pegged test stablecoin with no feed of its own;
        // value it against the USDC/USD feed (~$1).
        "AUSD" => USDC_FEED,
        _ => return None,
    };
    let hex_str = feed_hex.strip_prefix("0x").unwrap_or(feed_hex);
    let mut id = [0u8; 32];
    hex::decode_to_slice(hex_str, &mut id).ok()?;
    Some(id)
}

async fn account_exists(client: &arch_sdk::AsyncArchRpcClient, pubkey: &Pubkey) -> bool {
    client.read_account_info(*pubkey).await.is_ok()
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    dotenvy::dotenv().ok();
    let filter = EnvFilter::builder()
        .with_default_directive(tracing::Level::INFO.into())
        .from_env_lossy();
    tracing_subscriber::FmtSubscriber::builder()
        .with_env_filter(filter)
        .finish()
        .init();

    let args = Args::parse();
    let network = parse_network(&args.network)?;

    let autara_program_id = match args.program_id {
        Some(ref id) => parse_pubkey(id)?,
        None => autara_stage_program_id(),
    };
    let oracle_program_id = match args.oracle_program_id {
        Some(ref id) => parse_pubkey(id)?,
        None => autara_oracle_stage_program_id(),
    };

    // Load signer
    let signer_path = args
        .signer
        .or_else(|| std::env::var("AUTARA_SIGNER_KEY").ok())
        .context("Signer key required: use --signer or set AUTARA_SIGNER_KEY")?;
    let (signer_keypair, signer_pubkey) = with_secret_key_file(&signer_path)
        .context(format!("Failed to load signer key: {}", signer_path))?;

    tracing::info!("Program ID:        {:?}", autara_program_id);
    tracing::info!("Oracle Program ID: {:?}", oracle_program_id);
    tracing::info!("Signer:            {:?}", signer_pubkey);

    // Load tokens config
    let tokens_json = std::fs::read_to_string(&args.tokens)
        .context(format!("Failed to read tokens config: {}", args.tokens))?;
    let tokens_file: TokensFile =
        serde_json::from_str(&tokens_json).context("Failed to parse tokens config")?;
    let tokens = &tokens_file.tokens;
    tracing::info!("Loaded {} tokens from config", tokens.len());

    // Load per-token mint authority keypairs.
    let mut token_authorities: HashMap<String, (Keypair, Pubkey)> = HashMap::new();
    for (name, token) in tokens {
        let (kp, pk) = with_secret_key_file(&token.mint_authority_key_file).context(format!(
            "Failed to load mint authority key for {}: {}",
            name, token.mint_authority_key_file
        ))?;
        tracing::info!("Mint authority for {}: {:?}", name, pk);
        token_authorities.insert(name.clone(), (kp, pk));
    }

    // Load markets deploy plan (optional — server can run without creating any markets).
    let markets_plan: Option<MarketsFile> = match std::fs::read_to_string(&args.markets) {
        Ok(json) => Some(
            serde_json::from_str(&json)
                .context(format!("Failed to parse markets plan: {}", args.markets))?,
        ),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            tracing::warn!(
                "No markets plan at {} — skipping market creation",
                args.markets
            );
            None
        }
        Err(err) => {
            return Err(anyhow!(
                "Failed to read markets plan {}: {}",
                args.markets,
                err
            ))
        }
    };

    // Load + faucet-fund every unique curator referenced by the plan. Auto-generates the
    // key file if missing (via `with_secret_key_file`), so first run materializes the keys.
    let mut curators: HashMap<String, (Keypair, Pubkey)> = HashMap::new();
    if let Some(ref plan) = markets_plan {
        for entry in &plan.markets {
            if curators.contains_key(&entry.curator_key_file) {
                continue;
            }
            let (kp, pk) = with_secret_key_file(&entry.curator_key_file).context(format!(
                "Failed to load or generate curator key: {}",
                entry.curator_key_file
            ))?;
            tracing::info!("Curator {} -> {:?}", entry.curator_key_file, pk);
            curators.insert(entry.curator_key_file.clone(), (kp, pk));
        }
    }

    // Setup Arch client
    let config = ArchConfig {
        arch_node_url: args.arch_node.clone(),
        bitcoin_node_endpoint: String::new(),
        bitcoin_node_password: String::new(),
        bitcoin_node_username: String::new(),
    };
    let arch_client = config.arch_rpc_client();

    // Fund signer account via faucet (idempotent, needed for tx fees)
    tracing::info!("Funding signer account via faucet...");
    if let Err(e) = arch_client
        .create_and_fund_account_with_faucet(&signer_keypair)
        .await
    {
        tracing::warn!("Faucet funding failed (may already be funded): {:?}", e);
    }
    for (name, (kp, _)) in &token_authorities {
        tracing::info!("Funding mint authority for {} via faucet...", name);
        if let Err(e) = arch_client.create_and_fund_account_with_faucet(kp).await {
            tracing::warn!(
                "Faucet funding for {} authority failed (may already be funded): {:?}",
                name,
                e
            );
        }
    }
    for (path, (kp, _)) in &curators {
        tracing::info!("Funding curator {} via faucet...", path);
        if let Err(e) = arch_client.create_and_fund_account_with_faucet(kp).await {
            tracing::warn!(
                "Faucet funding for curator {} failed (may already be funded): {:?}",
                path,
                e
            );
        }
    }

    // Create the client for market creation
    let mut client = AutaraFullClientWithSigner::new_simple(
        arch_client.clone(),
        network,
        autara_program_id,
        signer_keypair,
    );

    // Load state
    tracing::info!("Loading protocol state...");
    tokio::time::timeout(Duration::from_secs(60), client.full_reload())
        .await
        .context("Timeout loading protocol state")??;
    tracing::info!("Protocol state loaded");

    // Create global config if not exists
    let global_config = client.read_client().get_global_config();
    if global_config.is_none() {
        tracing::info!("Creating global config...");
        client
            .create_global_config(signer_pubkey, signer_pubkey, 100)
            .await
            .context("Failed to create global config")?;
        tracing::info!("Global config created");
    } else {
        tracing::info!("Global config already exists");
    }

    let token_names: Vec<String> = tokens.keys().cloned().collect();

    // Create markets from the deploy plan (each market under its own curator key).
    if let Some(ref plan) = markets_plan {
        for entry in &plan.markets {
            let label = entry.name.as_deref().unwrap_or("(unnamed)");
            let supply_token = match tokens.get(&entry.supply) {
                Some(t) => t,
                None => {
                    tracing::error!(
                        "Market '{}': unknown supply token {} (not in tokens.json)",
                        label,
                        entry.supply
                    );
                    continue;
                }
            };
            let collateral_token = match tokens.get(&entry.collateral) {
                Some(t) => t,
                None => {
                    tracing::error!(
                        "Market '{}': unknown collateral token {} (not in tokens.json)",
                        label,
                        entry.collateral
                    );
                    continue;
                }
            };
            let supply_mint = parse_pubkey(&supply_token.mint)?;
            let collateral_mint = parse_pubkey(&collateral_token.mint)?;

            let supply_feed_id = match pyth_feed_for_token(&entry.supply) {
                Some(id) => id,
                None => {
                    tracing::error!(
                        "Market '{}': no Pyth feed mapping for {}",
                        label,
                        entry.supply
                    );
                    continue;
                }
            };
            let collateral_feed_id = match pyth_feed_for_token(&entry.collateral) {
                Some(id) => id,
                None => {
                    tracing::error!(
                        "Market '{}': no Pyth feed mapping for {}",
                        label,
                        entry.collateral
                    );
                    continue;
                }
            };

            let (curator_keypair, curator_pubkey) = curators
                .get(&entry.curator_key_file)
                .expect("curator preloaded above");

            let (market_pda, _) = find_market_pda(
                &autara_program_id,
                curator_pubkey,
                &supply_mint,
                &collateral_mint,
                entry.index,
            );

            if account_exists(&arch_client, &market_pda).await {
                tracing::info!(
                    "Market '{}' [{}/{} idx={} curator={:?}] already exists at {:?}",
                    label,
                    entry.supply,
                    entry.collateral,
                    entry.index,
                    curator_pubkey,
                    market_pda
                );
                continue;
            }

            let create_ix = CreateMarketInstruction {
                market_bump: 0,
                index: entry.index,
                ltv_config: entry.ltv_config.clone(),
                max_utilisation_rate: entry
                    .max_utilisation_rate
                    .unwrap_or_else(|| 0.9.into()),
                supply_oracle_config: OracleConfig::new_pyth(supply_feed_id, oracle_program_id),
                collateral_oracle_config: OracleConfig::new_pyth(
                    collateral_feed_id,
                    oracle_program_id,
                ),
                interest_rate: entry
                    .interest_rate
                    .clone()
                    .unwrap_or_else(InterestRateCurveKind::new_adaptive),
                lending_market_fee_in_bps: entry.lending_market_fee_in_bps.unwrap_or(500),
            };

            tracing::info!(
                "Creating market '{}' [{}/{} idx={} curator={:?}]...",
                label,
                entry.supply,
                entry.collateral,
                entry.index,
                curator_pubkey
            );
            let curator_client = client.with_signer(*curator_keypair);
            match curator_client
                .create_market(create_ix, supply_mint, collateral_mint)
                .await
            {
                Ok(market_pubkey) => {
                    tracing::info!(
                        "Market '{}' created at {:?}",
                        label,
                        market_pubkey
                    );
                }
                Err(e) => {
                    tracing::error!("Failed to create market '{}': {:?}", label, e);
                }
            }
        }
    }

    // Spawn Pyth feed pusher in background
    let mut feeds: Vec<String> = token_names
        .iter()
        .filter_map(|name| {
            let feed_hex = match name.to_uppercase().as_str() {
                "BTC" => Some(BTC_FEED),
                "USDC" => Some(USDC_FEED),
                "ETH" => Some(ETH_FEED),
                "USDT" => Some(USDT_FEED),
                // aUSD shares the USDC/USD feed (USD-pegged test stablecoin).
                "AUSD" => Some(USDC_FEED),
                _ => None,
            }?;
            Some(feed_hex.to_string())
        })
        .collect();
    // De-duplicate: multiple tokens (e.g. USDC + aUSD) may share one feed.
    feeds.sort();
    feeds.dedup();
    if !feeds.is_empty() {
        tracing::info!("Spawning Pyth feed pusher for {} feeds", feeds.len());
        let pusher_client = arch_client.clone();
        let pusher_signer = signer_keypair;
        let to_airdrop = signer_pubkey;
        tokio::spawn(async move {
            fetch_and_push_feeds(
                &pusher_client,
                &oracle_program_id,
                &pusher_signer,
                &feeds,
                network,
            )
            .await;
        });
        let aidrop_client = arch_client.clone();
        tokio::spawn(async move {
            loop {
                let _ = aidrop_client.request_airdrop(to_airdrop).await;
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
        });
    }

    // Build token minters from config (each mint uses its own authority).
    let mut minters = Vec::new();
    for (name, token) in tokens {
        let mint = parse_pubkey(&token.mint)?;
        let (authority_keypair, _) = token_authorities
            .get(name)
            .ok_or_else(|| anyhow!("Missing mint authority for token {}", name))?;
        let minter =
            TokenMinter::from_existing(arch_client.clone(), *authority_keypair, mint);
        tracing::info!("Token minter for {} ({:?})", name, mint);
        minters.push(minter);
    }

    // Start the shared state (auto-reloading)
    let autara_state =
        AutaraSharedState::new(arch_client.clone(), autara_program_id)
            .spawn()
            .0;

    // Build and start the RPC server
    let cors = CorsLayer::new()
        .allow_methods(Any)
        .allow_headers(Any)
        .allow_origin(Any);
    let server = Server::builder()
        .max_response_body_size(100 * 1024 * 1024) // 100 MB (before compression)
        .set_http_middleware(
            tower::ServiceBuilder::new()
                .timeout(Duration::from_secs(60))
                .layer(cors)
                .layer(CompressionLayer::new()),
        )
        .set_rpc_middleware(
            RpcServiceBuilder::new().layer(autara_client::api::tracing::AutaraTraceLayer),
        )
        .build(args.listen.parse::<SocketAddr>()?)
        .await?;
    let module = build_autara_server(autara_state.clone(), minters, arch_client).await?;
    let server_addr = server.local_addr()?;
    let handle = server.start(module);
    tracing::info!("Server running at: {}", server_addr);

    // Start Prometheus
    PrometheusExporter::launch(args.prometheus.parse()?, None).await?;
    tracing::info!("Prometheus exporter running at {}", args.prometheus);
    PrometheusAutaraIndexer::new(autara_state, Duration::from_secs(60)).spawn();

    handle.stopped().await;
    Ok(())
}
