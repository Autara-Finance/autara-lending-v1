use crate::fixture::autara_fixture::{
    AutaraFixture, LIQUIDATION_BONUS, LTV, MAX_UTILISATION_RATE, UNHEALTHY_LTV,
};
use autara_client::client::read::AutaraReadClient;
use autara_lib::{
    error::LendingError, interest_rate::interest_rate_kind::InterestRateCurveKind,
    ixs::CreateMarketInstruction, state::market_config::LtvConfig,
};

#[tokio::test]
async fn can_create_a_market() {
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
                lending_market_fee_in_bps: 0,
            },
            fixture.env().supply_mint,
            fixture.env().collateral_mint,
        )
        .await
        .unwrap();
    fixture.reload_market(&market).await;
    let market_data = fixture
        .curator_client()
        .read_client_ref()
        .get_market(&market)
        .unwrap();
    assert_eq!(
        market_data.market().config().curator(),
        fixture.curator_client().signer_pubkey()
    );
    assert_eq!(
        market_data.market().config().ltv_config(),
        &LtvConfig {
            max_ltv: LTV,
            unhealthy_ltv: UNHEALTHY_LTV,
            liquidation_bonus: LIQUIDATION_BONUS,
        },
    );
    assert_eq!(market_data.market().config().max_utilisation_rate(), 0.9);
}

#[tokio::test]
async fn cant_create_a_market_with_unhealthy_ltv_less_than_max_ltv() {
    let fixture = AutaraFixture::new().await;
    let err = fixture
        .curator_client()
        .create_market(
            CreateMarketInstruction {
                market_bump: 0,
                index: 0,
                ltv_config: LtvConfig {
                    max_ltv: LTV,
                    // unhealthy_ltv < max_ltv
                    unhealthy_ltv: 0.7.into(),
                    liquidation_bonus: LIQUIDATION_BONUS,
                },
                max_utilisation_rate: MAX_UTILISATION_RATE,
                supply_oracle_config: fixture.env().supply_oracle_config(),
                collateral_oracle_config: fixture.env().collateral_oracle_config(),
                interest_rate: InterestRateCurveKind::new_adaptive(),
                lending_market_fee_in_bps: 0,
            },
            fixture.env().supply_mint,
            fixture.env().collateral_mint,
        )
        .await;
    assert_eq!(err.unwrap_err(), LendingError::InvalidLtvConfig);
}

#[tokio::test]
async fn cant_create_a_market_when_unhealthy_ltv_times_liquidation_bonus_is_greater_can_max() {
    let fixture = AutaraFixture::new().await;
    let err = fixture
        .curator_client()
        .create_market(
            CreateMarketInstruction {
                market_bump: 0,
                index: 0,
                ltv_config: LtvConfig {
                    // unhealthy_ltv * LIQUIDATION_BONUS > 0.99
                    max_ltv: 0.98.into(),
                    unhealthy_ltv: UNHEALTHY_LTV,
                    liquidation_bonus: LIQUIDATION_BONUS,
                },
                max_utilisation_rate: MAX_UTILISATION_RATE,
                supply_oracle_config: fixture.env().supply_oracle_config(),
                collateral_oracle_config: fixture.env().collateral_oracle_config(),
                interest_rate: InterestRateCurveKind::new_adaptive(),
                lending_market_fee_in_bps: 0,
            },
            fixture.env().supply_mint,
            fixture.env().collateral_mint,
        )
        .await;
    assert_eq!(err.unwrap_err(), LendingError::InvalidLtvConfig);
}
