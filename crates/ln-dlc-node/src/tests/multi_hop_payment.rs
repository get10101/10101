use crate::node::Node;
use crate::tests::init_tracing;
use bitcoin::Amount;

#[tokio::test]
#[ignore]
async fn multi_hop_payment() {
    init_tracing();

    // Arrange

    let payer = Node::start_test_app("payer").await.unwrap();
    let coordinator = Node::start_test_coordinator("coordinator").await.unwrap();
    let payee = Node::start_test_app("payee").await.unwrap();

    payer.keep_connected(coordinator.info).await.unwrap();
    payee.keep_connected(coordinator.info).await.unwrap();

    coordinator.fund(Amount::from_sat(100_000)).await.unwrap();

    coordinator
        .open_channel(&payer, 20_000, 20_000)
        .await
        .unwrap();
    coordinator.open_channel(&payee, 20_000, 0).await.unwrap();

    let payer_balance_before = payer.get_ldk_balance();
    let coordinator_balance_before = coordinator.get_ldk_balance();
    let payee_balance_before = payee.get_ldk_balance();

    payer.sync();
    coordinator.sync();
    payee.sync();

    // Act

    let invoice_amount = 1_000;
    let invoice = payee.create_invoice(invoice_amount).unwrap();

    payer.send_payment(&invoice).unwrap();

    payee
        .wait_for_payment_claimed(invoice.payment_hash())
        .await
        .unwrap();

    // Assert

    // Sync LN wallet after payment is claimed to update the balances
    payer.sync();
    coordinator.sync();
    payee.sync();

    let payer_balance_after = payer.get_ldk_balance();
    let coordinator_balance_after = coordinator.get_ldk_balance();
    let payee_balance_after = payee.get_ldk_balance();

    let routing_fee = 1; // according to the default `ChannelConfig`

    assert_eq!(
        payer_balance_before.available - payer_balance_after.available - routing_fee,
        invoice_amount
    );

    assert_eq!(
        coordinator_balance_after.available - coordinator_balance_before.available,
        routing_fee
    );

    assert_eq!(
        payee_balance_after.available - payee_balance_before.available,
        invoice_amount
    );
}
