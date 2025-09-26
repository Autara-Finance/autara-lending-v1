use autara_client::client::tx_broadcast::AutaraClientError;
use autara_lib::{
    error::LendingError,
    event::AutaraEvent,
    ixs::{BorrowDepositAplInstruction, WithdrawRepayAplInstruction},
};

use crate::fixture::autara_fixture::AutaraFixture;

#[tokio::test]
async fn can_deposit_and_borrow() {
    let mut fixture = AutaraFixture::new().await;
    let market = fixture.create_market().await;
    fixture
        .user_two_client()
        .supply(&market, 1_000_000_000)
        .await
        .unwrap();
    fixture
        .user_client()
        .borrow_deposit(
            &market,
            BorrowDepositAplInstruction {
                deposit_amount: 1_000_000,
                borrow_amount: 2,
                ix_callback: None,
            },
        )
        .await
        .unwrap();
    fixture.reload_market(&market).await;
    let position = fixture
        .user_client()
        .get_borrow_position_health(&market)
        .unwrap();
    assert!(!position.ltv.is_zero());
}

#[tokio::test]
async fn cant_create_unhealthy_position_through_call_back() {
    let mut fixture = AutaraFixture::new().await;
    let market = fixture.create_market().await;
    fixture
        .user_two_client()
        .supply(&market, 1_000_000_000)
        .await
        .unwrap();
    let err = fixture
        .user_client()
        .borrow_deposit(
            &market,
            BorrowDepositAplInstruction {
                deposit_amount: 100,
                borrow_amount: 0,
                ix_callback: Some(
                    fixture
                        .user_client()
                        .tx_builder()
                        .borrow(&market, 100_000_000)
                        .await
                        .unwrap()
                        .instructions
                        .last()
                        .unwrap()
                        .clone(),
                ),
            },
        )
        .await
        .unwrap_err();
    let AutaraClientError::AutaraTxError { events, .. } = &err else {
        panic!("Expected AutaraTxError, got {err}");
    };
    // ensure first deposit borrow did happen
    assert!(matches!(events.events[0], AutaraEvent::BorrowAndDeposit(_)));
    assert_eq!(err, LendingError::MaxLtvReached);
}

#[tokio::test]
async fn can_deposit_and_borrow_then_withdraw_repay() {
    let mut fixture = AutaraFixture::new().await;
    let market = fixture.create_market().await;
    fixture
        .user_two_client()
        .supply(&market, 1_000_000_000)
        .await
        .unwrap();
    fixture
        .user_client()
        .borrow_deposit(
            &market,
            BorrowDepositAplInstruction {
                deposit_amount: 1_000_000,
                borrow_amount: 2,
                ix_callback: None,
            },
        )
        .await
        .unwrap();
    fixture
        .user_client()
        .withdraw_repay(
            &market,
            WithdrawRepayAplInstruction {
                repay_amount: 0,
                withdraw_amount: 0,
                repay_all: true,
                withdraw_all: true,
                ix_callback: None,
            },
        )
        .await
        .unwrap();
    fixture.reload_market(&market).await;
    let position = fixture
        .user_client()
        .get_borrow_position_health(&market)
        .unwrap();
    assert!(position.ltv.is_zero());
}
