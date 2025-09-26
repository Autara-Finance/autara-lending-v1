use autara_client::client::read::AutaraReadClient;
use autara_lib::{
    error::LendingError, ixs::UpdateConfigInstruction, oracle::oracle_config::OracleConfig,
};
use autara_program::error::LendingAccountValidationError;

use crate::fixture::autara_fixture::AutaraFixture;

#[tokio::test]
async fn can_update_market() {
    let mut fixture = AutaraFixture::new().await;
    let market = fixture.create_market().await;
    let update = UpdateConfigInstruction {
        ltv_config: Some(autara_lib::state::market_config::LtvConfig {
            max_ltv: 0.85.into(),
            unhealthy_ltv: 0.95.into(),
            liquidation_bonus: 0.005.into(),
        }),
        max_supply_atoms: Some(123),
        max_utilisation_rate: Some(0.6.into()),
        ..Default::default()
    };
    fixture
        .curator_client()
        .update_config(&market, update.clone())
        .await
        .unwrap();
    fixture.reload_market(&market).await;
    let market_data = fixture
        .curator_client()
        .read_client_ref()
        .get_market(&market)
        .unwrap();
    assert_eq!(
        market_data.market().config().ltv_config(),
        &update.ltv_config.unwrap()
    );
    assert_eq!(
        market_data.market().config().max_supply_atoms(),
        update.max_supply_atoms.unwrap()
    );
    assert_eq!(
        market_data.market().config().max_utilisation_rate(),
        update.max_utilisation_rate.unwrap()
    );
}

#[tokio::test]
async fn only_curator_can_update_market() {
    let mut fixture = AutaraFixture::new().await;
    let market = fixture.create_market().await;
    let update = UpdateConfigInstruction {
        ltv_config: Some(autara_lib::state::market_config::LtvConfig {
            max_ltv: 0.85.into(),
            unhealthy_ltv: 0.95.into(),
            liquidation_bonus: 0.005.into(),
        }),
        max_supply_atoms: Some(123),
        max_utilisation_rate: Some(0.6.into()),
        ..Default::default()
    };
    let err = fixture
        .user_client()
        .update_config(&market, update)
        .await
        .unwrap_err();
    assert_eq!(err, LendingAccountValidationError::InvalidMarketAuthority);
}

#[tokio::test]
async fn cant_update_with_invalid_supply_oracle_config() {
    let mut fixture = AutaraFixture::new().await;
    let market = fixture.create_market().await;

    let config = UpdateConfigInstruction {
        supply_oracle_config: Some(OracleConfig::new_pyth([8; 32], Default::default())),
        collateral_oracle_config: None,
        ..Default::default()
    };
    let err = fixture
        .curator_client()
        .update_config(&market, config)
        .await
        .unwrap_err();
    assert_eq!(err, LendingError::InvalidPythOracleAccount);
}

#[tokio::test]
async fn cant_update_with_invalid_collateral_oracle_config() {
    let mut fixture = AutaraFixture::new().await;
    let market = fixture.create_market().await;
    let config = UpdateConfigInstruction {
        collateral_oracle_config: Some(OracleConfig::new_pyth([8; 32], Default::default())),
        supply_oracle_config: None,
        ..Default::default()
    };
    let err = fixture
        .curator_client()
        .update_config(&market, config)
        .await
        .unwrap_err();
    assert_eq!(err, LendingError::InvalidPythOracleAccount);
}

#[tokio::test]
async fn can_update_global_config() {
    let mut fixture = AutaraFixture::new().await;
    fixture
        .admin_client()
        .update_global_config(autara_lib::ixs::UpdateGlobalConfigInstruction {
            protocol_fee_share_in_bps: Some(123),
            ..Default::default()
        })
        .await
        .unwrap();
    fixture.reload().await;
    let config = fixture
        .user_client()
        .read_client()
        .get_global_config()
        .unwrap();
    assert_eq!(config.protocol_fee_share_in_bps(), 123);
}

#[tokio::test]
async fn only_admin_can_update_global_config() {
    let fixture = AutaraFixture::new().await;
    let err = fixture
        .user_client()
        .update_global_config(autara_lib::ixs::UpdateGlobalConfigInstruction {
            protocol_fee_share_in_bps: Some(123),
            ..Default::default()
        })
        .await
        .unwrap_err();
    assert_eq!(err, LendingAccountValidationError::InvalidProtocolAuthority);
}

#[tokio::test]
async fn only_new_nominated_can_accept_nomination() {
    let fixture = AutaraFixture::new().await;
    fixture
        .admin_client()
        .update_global_config(autara_lib::ixs::UpdateGlobalConfigInstruction {
            nominated_admin: Some(*fixture.admin_client().signer_pubkey()),
            ..Default::default()
        })
        .await
        .unwrap();
    let err = fixture
        .user_client()
        .update_global_config(autara_lib::ixs::UpdateGlobalConfigInstruction {
            accept_nomination: true,
            ..Default::default()
        })
        .await
        .unwrap_err();
    assert_eq!(err, LendingAccountValidationError::InvalidProtocolAuthority);
}
