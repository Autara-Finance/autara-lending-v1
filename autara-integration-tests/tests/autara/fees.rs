use autara_client::client::read::AutaraReadClient;
use autara_lib::{
    interest_rate::interest_rate_kind::InterestRateCurveKind, ixs::CreateMarketInstruction,
    state::market_config::LtvConfig,
};

use crate::fixture::autara_fixture::{
    AutaraFixture, BTC, LIQUIDATION_BONUS, LTV, MAX_UTILISATION_RATE, UNHEALTHY_LTV, USDC,
};

#[tokio::test]
async fn can_collect_curator_fees() {
    let mut fixture = AutaraFixture::new().await;
    let market = fixture
        .curator_client()
        .create_market(
            CreateMarketInstruction {
                market_bump: 0,
                index: 0,
                ltv_config: LtvConfig {
                    max_ltv: LTV,
                    unhealthy_ltv: UNHEALTHY_LTV,
                    liquidation_bonus: LIQUIDATION_BONUS,
                },
                max_utilisation_rate: MAX_UTILISATION_RATE,
                supply_oracle_config: fixture.env().supply_oracle_config(),
                collateral_oracle_config: fixture.env().collateral_oracle_config(),
                interest_rate: InterestRateCurveKind::new_adaptive(),
                lending_market_fee_in_bps: 1000,
            },
            fixture.env().supply_mint,
            fixture.env().collateral_mint,
        )
        .await
        .unwrap();
    fixture.reload_market(&market).await;
    fixture
        .user_client()
        .supply(&market, USDC(1000000.))
        .await
        .unwrap();
    fixture
        .user_client()
        .deposit_collateral(&market, BTC(10000.))
        .await
        .unwrap();
    fixture
        .user_client()
        .borrow(&market, USDC(800000.))
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    let balance_before = fixture
        .fetch_balance(fixture.curator_client().signer_pubkey())
        .await;
    fixture.reload_market(&market).await;
    let market_w = fixture
        .user_client()
        .read_client()
        .get_market(&market)
        .unwrap();
    let snapshot = market_w.market().supply_vault().get_summary().unwrap();
    assert!(snapshot.pending_curator_fee_atoms != 0);
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    fixture
        .curator_client()
        .reedeem_curator_fees(&market)
        .await
        .unwrap();
    let balance_after = fixture
        .fetch_balance(fixture.curator_client().signer_pubkey())
        .await;
    let diff = balance_after.delta(&balance_before);
    assert!(diff.supply > snapshot.pending_curator_fee_atoms as i64);
}

#[tokio::test]
async fn can_collect_admin_fees() {
    let mut fixture = AutaraFixture::new().await;
    let market = fixture
        .curator_client()
        .create_market(
            CreateMarketInstruction {
                market_bump: 0,
                index: 0,
                ltv_config: LtvConfig {
                    max_ltv: LTV,
                    unhealthy_ltv: UNHEALTHY_LTV,
                    liquidation_bonus: LIQUIDATION_BONUS,
                },
                max_utilisation_rate: MAX_UTILISATION_RATE,
                supply_oracle_config: fixture.env().supply_oracle_config(),
                collateral_oracle_config: fixture.env().collateral_oracle_config(),
                interest_rate: InterestRateCurveKind::new_approximate_fixed_apy(1000000.),
                lending_market_fee_in_bps: 2000,
            },
            fixture.env().supply_mint,
            fixture.env().collateral_mint,
        )
        .await
        .unwrap();
    fixture.reload().await;
    fixture
        .user_client()
        .supply(&market, USDC(1000000.))
        .await
        .unwrap();
    fixture
        .user_client()
        .deposit_collateral(&market, BTC(10000.))
        .await
        .unwrap();
    fixture
        .user_client()
        .borrow(&market, USDC(800000.))
        .await
        .unwrap();
    let balance_before = fixture
        .fetch_balance(fixture.admin_client().signer_pubkey())
        .await;
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    fixture.reload_market(&market).await;
    let market_w = fixture
        .user_client()
        .read_client()
        .get_market(&market)
        .unwrap();
    let snapshot = market_w.market().supply_vault().get_summary().unwrap();
    assert!(snapshot.pending_protocol_fee_atoms != 0);
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    fixture
        .admin_client()
        .reedeem_protocol_fees(&market)
        .await
        .unwrap();
    let balance_after = fixture
        .fetch_balance(fixture.admin_client().signer_pubkey())
        .await;
    let diff = balance_after.delta(&balance_before);
    assert!(diff.supply > snapshot.pending_protocol_fee_atoms as i64);
}
