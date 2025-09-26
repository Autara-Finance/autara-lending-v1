use arch_sdk::arch_program::{bitcoin::Network, pubkey::Pubkey};
use autara_client::{
    client::{
        client_with_signer::AutaraFullClientWithSigner, read::AutaraReadClient,
        tx_broadcast::AutaraClientError,
    },
    config::{autara_oracle_stage_program_id, autara_stage_program_id, ArchConfig},
    test::AutaraTestEnv,
};
use autara_lib::{
    interest_rate::interest_rate_kind::InterestRateCurveKind, ixs::CreateMarketInstruction,
    oracle::oracle_config::OracleConfig, state::market_config::LtvConfig,
};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), AutaraClientError> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(tracing::Level::INFO.into())
                .from_env_lossy(),
        )
        .init();
    let config = ArchConfig::dev();

    let arch_client = config.arch_rpc_client();
    let test_env = AutaraTestEnv::new(
        arch_client.clone(),
        autara_stage_program_id(),
        autara_oracle_stage_program_id(),
    )
    .await?;

    let mut autara_client = AutaraFullClientWithSigner::new_simple(
        arch_client,
        Network::Testnet,
        test_env.autara_program_pubkey,
        test_env.autara_oracle_program_pubkey,
        test_env.user_keypair,
    );

    autara_client
        .create_global_config(Pubkey::new_unique(), Pubkey::new_unique(), 0)
        .await?;

    test_env.push_collateral_price(100_000.).await?;
    test_env.push_supply_price(1.).await?;

    let market = autara_client
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

    autara_client
        .reload_authority_accounts_for_market(&market)
        .await?;

    println!(
        "{:#?}",
        autara_client
            .read_client()
            .get_market(&market)
            .unwrap()
            .market()
    );

    autara_client.supply(&market, 100_000_000_000_000).await?; // 100k USDC

    autara_client
        .reload_authority_accounts_for_market(&market)
        .await?;

    println!("{:#?}", autara_client.get_supply_position(&market).unwrap());

    autara_client
        .deposit_collateral(&market, 1_000_000_000) // 1 BTC
        .await?;

    autara_client
        .borrow(&market, 40_000_000_000_000) // 40k USDC
        .await?;

    autara_client
        .reload_authority_accounts_for_market(&market)
        .await?;

    println!("{:#?}", autara_client.get_borrow_position_health(&market)?);

    autara_client
        .repay(&market, Some(5_000_000_000_000)) // 5k USDC
        .await?;

    autara_client
        .reload_authority_accounts_for_market(&market)
        .await?;

    println!("{:#?}", autara_client.get_borrow_position_health(&market)?);

    autara_client
        .withdraw_supply(&market, Some(50_000_000_000_000)) // 50k USDC
        .await?;

    autara_client
        .reload_authority_accounts_for_market(&market)
        .await?;

    println!("{:#?}", autara_client.get_supply_position(&market).unwrap());

    test_env.push_collateral_price(39_000.).await?;

    autara_client
        .reload_authority_accounts_for_market(&market)
        .await?;

    println!("{:#?}", autara_client.get_borrow_position_health(&market)?);

    let position = autara_client
        .read_client()
        .get_borrow_position(&market, autara_client.signer_pubkey())
        .0;
    autara_client
        .liquidate(&market, &position, None, None, None)
        .await?;

    autara_client
        .reload_authority_accounts_for_market(&market)
        .await?;

    println!("{:#?}", autara_client.get_borrow_position_health(&market)?);

    Ok(())
}
