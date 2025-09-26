use autara_lib::error::LendingError;

use crate::fixture::autara_fixture::{AutaraFixture, BTC, USDC};

#[tokio::test]
async fn can_borrow() {
    let mut fixture = AutaraFixture::new().await;
    let market = fixture.create_market().await;
    let deposit = USDC(1_000.);
    fixture
        .user_client()
        .supply(&market, deposit)
        .await
        .unwrap();
    fixture
        .user_client()
        .deposit_collateral(&market, BTC(0.1))
        .await
        .unwrap();
    let balance_before = fixture.fetch_user_balance().await;
    fixture
        .user_client()
        .borrow(&market, USDC(0.1))
        .await
        .unwrap();
    let balance_after = fixture.fetch_user_balance().await;
    let diff = balance_after.delta(&balance_before);
    assert_eq!(diff.supply, USDC(0.1) as i64);
    fixture.reload_market(&market).await;
    let position = fixture
        .user_client()
        .get_borrow_position_health(&market)
        .unwrap();
    assert!(position.borrowed_atoms >= USDC(0.1) && position.borrowed_atoms <= USDC(0.1) + 1);
    assert_eq!(position.collateral_atoms, BTC(0.1));
}

#[tokio::test]
async fn can_repay() {
    let mut fixture = AutaraFixture::new().await;
    let market = fixture.create_market().await;

    let deposit = USDC(1_000_000.);
    fixture
        .user_client()
        .supply(&market, deposit)
        .await
        .unwrap();
    fixture
        .user_client()
        .deposit_collateral(&market, BTC(1.))
        .await
        .unwrap();
    fixture
        .user_client()
        .borrow(&market, USDC(50_000.))
        .await
        .unwrap();
    fixture.reload_market(&market).await;
    let balance_before = fixture.fetch_user_balance().await;
    fixture.user_client().repay(&market, None).await.unwrap();
    let balance_after = fixture.fetch_user_balance().await;
    fixture.reload_market(&market).await;
    let diff = balance_after.delta(&balance_before);
    assert!(-diff.supply > USDC(50_000.) as i64);
    fixture.reload_market(&market).await;
    let position = fixture
        .user_client()
        .get_borrow_position_health(&market)
        .unwrap();
    assert_eq!(position.borrowed_atoms, USDC(0.));
    assert_eq!(position.collateral_atoms, BTC(1.));
}

#[tokio::test]
async fn cant_borrow_more_than_max_ltv() {
    let mut fixture = AutaraFixture::new().await;
    let market = fixture.create_market().await;

    let deposit = USDC(1_000_000.);
    fixture
        .user_client()
        .supply(&market, deposit)
        .await
        .unwrap();
    fixture
        .user_client()
        .deposit_collateral(&market, BTC(1.))
        .await
        .unwrap();
    let err = fixture
        .user_client()
        .borrow(&market, USDC(100000000.))
        .await
        .unwrap_err();
    assert_eq!(err, LendingError::MaxLtvReached)
}

#[tokio::test]
async fn cant_withdraw_more_than_max_ltv() {
    let mut fixture = AutaraFixture::new().await;
    let market = fixture.create_market().await;

    let deposit = USDC(1_000_000.);
    fixture
        .user_client()
        .supply(&market, deposit)
        .await
        .unwrap();
    fixture
        .user_client()
        .deposit_collateral(&market, BTC(1.))
        .await
        .unwrap();
    fixture
        .user_client()
        .borrow(&market, USDC(10_000.))
        .await
        .unwrap();
    let err = fixture
        .user_client()
        .withdraw_collateral(&market, Some(BTC(0.9)))
        .await
        .unwrap_err();
    assert_eq!(err, LendingError::MaxLtvReached);
    let err = fixture
        .user_client()
        .withdraw_collateral(&market, None)
        .await
        .unwrap_err();
    assert_eq!(err, LendingError::MaxLtvReached);
}

#[tokio::test]
async fn cant_borrow_more_than_max_utilisation_rate() {
    let mut fixture = AutaraFixture::new().await;
    let market = fixture.create_market().await;

    let deposit = USDC(1_000_000.);
    fixture
        .user_client()
        .supply(&market, deposit)
        .await
        .unwrap();
    fixture
        .user_client()
        .deposit_collateral(&market, BTC(10000.))
        .await
        .unwrap();
    let err = fixture
        .user_client()
        .borrow(&market, deposit - 1)
        .await
        .unwrap_err();
    assert_eq!(err, LendingError::MaxUtilisationRateReached)
}
