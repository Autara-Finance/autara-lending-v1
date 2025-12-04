use autara_lib::error::LendingError;

use crate::fixture::autara_fixture::AutaraFixture;

#[tokio::test]
async fn can_deposit_collateral() {
    let mut fixture = AutaraFixture::new().await;
    let market = fixture.create_market().await;

    let deposit = 10000;
    let balance_before = fixture.fetch_user_balance().await;
    dbg!(&balance_before);
    fixture
        .user_client()
        .deposit_collateral(&market, deposit)
        .await
        .unwrap();
    let balance_after = fixture.fetch_user_balance().await;
    let diff = balance_after.delta(&balance_before);
    assert_eq!(-diff.collateral, deposit as i64);
    fixture.reload_market(&market).await;
    let position = fixture
        .user_client()
        .get_borrow_position_health(&market)
        .unwrap();
    assert_eq!(position.collateral_atoms, deposit)
}

#[tokio::test]
async fn can_withraw_collateral() {
    let mut fixture = AutaraFixture::new().await;
    let market = fixture.create_market().await;

    let deposit = 10000;
    fixture
        .user_client()
        .deposit_collateral(&market, deposit)
        .await
        .unwrap();
    let balance_before = fixture.fetch_user_balance().await;
    fixture.reload_market(&market).await;
    fixture
        .user_client()
        .withdraw_collateral(&market, None)
        .await
        .unwrap();
    let balance_after = fixture.fetch_user_balance().await;
    let diff = balance_after.delta(&balance_before);
    assert_eq!(diff.collateral, deposit as i64);
    fixture.reload_market(&market).await;
    let position = fixture
        .user_client()
        .get_borrow_position_health(&market)
        .unwrap();
    assert_eq!(position.collateral_atoms, 0);
}

#[tokio::test]
pub async fn cant_withdraw_more_than_deposited() {
    let mut fixture = AutaraFixture::new().await;
    let market = fixture.create_market().await;

    let deposit = 10000;
    fixture
        .user_client()
        .deposit_collateral(&market, deposit)
        .await
        .unwrap();
    fixture.reload_market(&market).await;
    let err = fixture
        .user_client()
        .withdraw_collateral(&market, Some(deposit + 1))
        .await
        .unwrap_err();
    assert_eq!(err, LendingError::WithdrawalExceedsDeposited);
}
