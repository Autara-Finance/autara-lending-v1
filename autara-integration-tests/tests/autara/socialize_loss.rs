use autara_client::client::read::AutaraReadClient;
use autara_lib::{
    event::{AutaraEvent, SingleMarketTransactionEvent},
    pda::find_borrow_position_pda,
};
use autara_program::error::LendingAccountValidationError;

use crate::fixture::autara_fixture::AutaraFixture;

#[tokio::test]
async fn only_curator_can_socialize_loss() {
    let mut fixture = AutaraFixture::new().await;
    let market = fixture.create_market().await;
    fixture
        .user_two_client()
        .supply(&market, 100000000)
        .await
        .unwrap();
    fixture
        .user_client()
        .deposit_collateral(&market, 100000000)
        .await
        .unwrap();
    fixture.user_client().borrow(&market, 100).await.unwrap();
    let err = fixture
        .user_client()
        .socialize_loss(
            &market,
            &find_borrow_position_pda(
                &fixture.env().autara_program_pubkey,
                &market,
                fixture.user_client().signer_pubkey(),
            )
            .0,
        )
        .await
        .unwrap_err();
    assert_eq!(err, LendingAccountValidationError::InvalidMarketAuthority);
}

#[tokio::test]
async fn curator_can_socialize_loss() {
    let mut fixture = AutaraFixture::new().await;
    let market = fixture.create_market().await;
    fixture
        .user_two_client()
        .supply(&market, 100000000)
        .await
        .unwrap();
    fixture
        .user_client()
        .deposit_collateral(&market, 1000000)
        .await
        .unwrap();
    fixture.user_client().borrow(&market, 10000).await.unwrap();
    fixture.env().push_supply_price(10000000000.).await.unwrap();
    fixture.env().push_collateral_price(0.1).await.unwrap();
    fixture.reload().await;
    let position = fixture
        .user_client()
        .get_borrow_position_health(&market)
        .unwrap();
    assert!(position.ltv > 1.into());
    let curator_balance_before_socialize = fixture
        .fetch_balance(fixture.curator_client().signer_pubkey())
        .await;
    fixture
        .curator_client()
        .socialize_loss(
            &market,
            &find_borrow_position_pda(
                &fixture.env().autara_program_pubkey,
                &market,
                fixture.user_client().signer_pubkey(),
            )
            .0,
        )
        .await
        .unwrap();
    fixture.reload().await;
    let curator_balance_after_socialize = fixture
        .fetch_balance(fixture.curator_client().signer_pubkey())
        .await;
    let position = fixture
        .user_two_client()
        .get_supply_position(&market)
        .unwrap();
    let market = fixture
        .user_client()
        .read_client()
        .get_market(&market)
        .unwrap();
    assert_eq!(
        market.market().supply_position_info(&position).unwrap(),
        100000000 - 10000 - 1 // rounding down error
    );
    let diff = curator_balance_after_socialize.delta(&curator_balance_before_socialize);
    assert_eq!(diff.collateral, 1000000);
    assert_eq!(diff.supply, 0);
}

#[tokio::test]
async fn can_donate() {
    let mut fixture = AutaraFixture::new().await;
    let market = fixture.create_market().await;
    fixture
        .user_two_client()
        .supply(&market, 100000000)
        .await
        .unwrap();
    fixture
        .user_client()
        .donate_supply(&market, 1000000)
        .await
        .unwrap();
    let event = fixture
        .user_two_client()
        .withdraw_supply(&market, None)
        .await
        .unwrap();
    let AutaraEvent::Withdraw(SingleMarketTransactionEvent { amount, .. }) = &event.events[0]
    else {
        panic!("Expected WithdrawSupply event");
    };
    assert!(*amount >= 100000000 + 1000000 - 1); // rounding down error
}
