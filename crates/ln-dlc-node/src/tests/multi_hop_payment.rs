use bitcoin::Amount;

use crate::node::Node;
use crate::tests::init_tracing;
use std::time::Duration;

#[tokio::test]
#[ignore]
async fn multi_hop_payment() {
    init_tracing();

    // Arrange

    let payer = Node::start_test_app("payer").await.unwrap();
    let router = Node::start_test_coordinator("router").await.unwrap();
    let payee = Node::start_test_app("payee").await.unwrap();

    payer.keep_connected(router.info).await.unwrap();
    payee.keep_connected(router.info).await.unwrap();

    payer.fund(Amount::from_sat(50_000)).await.unwrap();
    router.fund(Amount::from_sat(100_000)).await.unwrap();

    router
        .open_channel(&payer.info, 20_000, 20_000)
        .await
        .unwrap();
    router
        .open_channel(&payee.info, 20_000, 20_000)
        .await
        .unwrap();

    let payer_balance_before = payer.get_ldk_balance();
    let router_balance_before = router.get_ldk_balance();
    let payee_balance_before = payee.get_ldk_balance();

    payer.sync();
    router.sync();
    payee.sync();

    // For the channels to be announced
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Act

    let invoice_amount = 1_000;
    let invoice = payee.create_invoice(invoice_amount).unwrap();

    payer.send_payment(&invoice).unwrap();

    // For the payment to be claimed before the wallet syncs
    tokio::time::sleep(Duration::from_secs(1)).await;

    payer.sync();
    router.sync();
    payee.sync();

    // Assert

    let payer_balance_after = payer.get_ldk_balance();
    let router_balance_after = router.get_ldk_balance();
    let payee_balance_after = payee.get_ldk_balance();

    let routing_fee = 1; // according to the default `ChannelConfig`

    assert_eq!(
        payer_balance_before.available - payer_balance_after.available - routing_fee,
        invoice_amount
    );

    assert_eq!(
        router_balance_after.available - router_balance_before.available,
        routing_fee
    );

    assert_eq!(
        payee_balance_after.available - payee_balance_before.available,
        invoice_amount
    );
}
