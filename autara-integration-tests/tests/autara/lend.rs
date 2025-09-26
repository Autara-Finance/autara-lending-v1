use autara_lib::{error::LendingError, event::AutaraEvent, ixs::UpdateConfigInstruction};

use crate::fixture::autara_fixture::AutaraFixture;

#[tokio::test]
async fn can_deposit() {
    let mut fixture = AutaraFixture::new().await;
    let market = fixture.create_market().await;

    let deposit = 10000;
    let balance_before = fixture.fetch_user_balance().await;
    fixture
        .user_client()
        .supply(&market, deposit)
        .await
        .unwrap();
    let balance_after = fixture.fetch_user_balance().await;
    let diff = balance_after.delta(&balance_before);
    assert_eq!(-diff.supply, deposit as i64);
    fixture.reload_market(&market).await;
    let position = fixture.user_client().get_supply_position(&market).unwrap();
    assert_eq!(position.deposited_atoms(), deposit)
}

#[tokio::test]
async fn can_withdraw() {
    let mut fixture = AutaraFixture::new().await;
    let market = fixture.create_market().await;

    let deposit = 10000;
    fixture
        .user_client()
        .supply(&market, deposit)
        .await
        .unwrap();
    let balance_before = fixture.fetch_user_balance().await;
    fixture.reload_market(&market).await;
    fixture
        .user_client()
        .withdraw_supply(&market, None)
        .await
        .unwrap();
    let balance_after = fixture.fetch_user_balance().await;
    let diff = balance_after.delta(&balance_before);
    assert_eq!(diff.supply, deposit as i64);
    fixture.reload_market(&market).await;
    let position = fixture.user_client().get_supply_position(&market).unwrap();
    assert_eq!(position.deposited_atoms(), 0);
    assert_eq!(position.shares(), 0);
}

#[tokio::test]
async fn cant_withdraw_more_than_deposited() {
    let mut fixture = AutaraFixture::new().await;
    let market = fixture.create_market().await;
    let deposit = 10000;
    fixture
        .user_client()
        .supply(&market, deposit)
        .await
        .unwrap();
    fixture
        .user_two_client()
        .supply(&market, deposit)
        .await
        .unwrap();
    fixture.reload_market(&market).await;
    let err = fixture
        .user_client()
        .withdraw_supply(&market, Some(deposit + 1))
        .await
        .unwrap_err();
    assert_eq!(err, LendingError::WithdrawalExceedsDeposited)
}

#[tokio::test]
async fn cant_supply_more_than_supply_limit() {
    let mut fixture = AutaraFixture::new().await;
    let market = fixture.create_market().await;

    let limit = 10_000;
    fixture
        .curator_client()
        .update_config(
            &market,
            UpdateConfigInstruction {
                max_supply_atoms: Some(limit),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let err = fixture
        .user_client()
        .supply(&market, limit + 1)
        .await
        .unwrap_err();
    assert_eq!(err, LendingError::MaxSupplyReached);
}

#[tokio::test]
async fn can_donate() {
    let mut fixture = AutaraFixture::new().await;
    let market = fixture.create_market().await;
    fixture.user_client().supply(&market, 10_000).await.unwrap();
    fixture
        .user_client()
        .donate_supply(&market, 5_000)
        .await
        .unwrap();
    let event = fixture
        .user_client()
        .withdraw_supply(&market, None)
        .await
        .unwrap()
        .events;
    let AutaraEvent::Withdraw(event) = event.last().unwrap() else {
        panic!("Expected WithdrawSupply event");
    };
    assert_eq!(event.amount, 15_000);
}
