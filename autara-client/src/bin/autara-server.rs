use arch_sdk::arch_program::pubkey::Pubkey;

use autara_client::api::server::build_autara_server;
use autara_client::client::client_with_signer::AutaraFullClientWithSigner;
use autara_client::client::shared_autara_state::AutaraSharedState;
use autara_client::config::{autara_oracle_stage_program_id, autara_stage_program_id, ArchConfig};
use autara_client::prometheus::autara_indexer::PrometheusAutaraIndexer;
use autara_client::prometheus::exporter::PrometheusExporter;
use autara_client::test::AutaraTestEnv;
use autara_lib::ixs::CreateMarketInstruction;
use autara_lib::state::market_config::LtvConfig;
use autara_lib::{
    interest_rate::interest_rate_kind::InterestRateCurveKind, oracle::oracle_config::OracleConfig,
};
use autara_pyth::{BTC_FEED, USDC_FEED};
use jsonrpsee::server::{RpcServiceBuilder, Server};
use std::net::SocketAddr;
use std::time::Duration;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let filter = EnvFilter::builder()
        .with_default_directive(tracing::Level::INFO.into())
        .from_env_lossy();
    tracing_subscriber::FmtSubscriber::builder()
        .with_env_filter(filter)
        .finish()
        .init();
    let config = ArchConfig::dev();
    let arch_client = config.arch_rpc_client();
    let mut test_env = AutaraTestEnv::new(
        arch_client.clone(),
        autara_stage_program_id(),
        autara_oracle_stage_program_id(),
    )
    .await?;
    let btc: [u8; 32] = hex::decode(&BTC_FEED[2..]).unwrap().try_into().unwrap();
    let usdc: [u8; 32] = hex::decode(&USDC_FEED[2..]).unwrap().try_into().unwrap();
    test_env.supply_feed_id = usdc;
    test_env.collateral_feed_id = btc;

    test_env.push_collateral_price(100_000.).await?;
    test_env.push_supply_price(1.).await?;

    test_env.spawn_pyth_pusher();

    tracing::info!(
        "Authority {:?}",
        Pubkey::from_slice(&test_env.authority_keypair.x_only_public_key().0.serialize())
    );

    let arch_client = config.arch_rpc_client();
    let autara_client = AutaraSharedState::new(
        arch_client.clone(),
        test_env.autara_program_pubkey,
        test_env.autara_oracle_program_pubkey,
    )
    .spawn()
    .0;
    let temp_client = AutaraFullClientWithSigner::new(
        autara_client.clone(),
        arch_client.clone(),
        arch_sdk::arch_program::bitcoin::Network::Regtest,
        test_env.authority_keypair,
    );
    let market = temp_client
        .create_market(
            CreateMarketInstruction {
                market_bump: 0,
                index: 0,
                ltv_config: LtvConfig {
                    max_ltv: 0.8.into(),
                    unhealthy_ltv: 0.9.into(),
                    liquidation_bonus: 0.05.into(),
                },
                max_utilisation_rate: 0.9.into(),
                supply_oracle_config: OracleConfig::new_pyth(
                    test_env.supply_feed_id,
                    test_env.autara_oracle_program_pubkey,
                ),
                collateral_oracle_config: OracleConfig::new_pyth(
                    test_env.collateral_feed_id,
                    test_env.autara_oracle_program_pubkey,
                ),
                interest_rate: InterestRateCurveKind::new_adaptive(),
                lending_market_fee_in_bps: 100,
            },
            test_env.supply_mint,
            test_env.collateral_mint,
        )
        .await?;
    temp_client.read_client().reload().await?;
    temp_client
        .with_signer(test_env.user_keypair)
        .supply(&market, 500_000 * 10u64.pow(9))
        .await?;
    tracing::info!("market = {:?}", market);
    let cors = CorsLayer::new()
        .allow_methods(Any)
        .allow_headers(Any)
        .allow_origin(Any);
    let server = Server::builder()
        .set_http_middleware(tower::ServiceBuilder::new().layer(cors))
        .set_rpc_middleware(
            RpcServiceBuilder::new().layer(autara_client::api::tracing::AutaraTraceLayer),
        )
        .build("0.0.0.0:62776".parse::<SocketAddr>()?)
        .await?;
    let module = build_autara_server(autara_client.clone(), test_env, arch_client).await?;
    let server_addr = server.local_addr()?;
    let handle = server.start(module);
    tracing::info!("Server running at: {}", server_addr);
    PrometheusExporter::launch("0.0.0.0:62777".parse()?, None).await?;
    tracing::info!("Prometheus exporter running on port 62777");
    PrometheusAutaraIndexer::new(autara_client, Duration::from_secs(60)).spawn();
    handle.stopped().await;
    Ok(())
}
