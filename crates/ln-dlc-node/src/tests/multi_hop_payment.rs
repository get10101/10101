use crate::node::Node;
use crate::tests::init_tracing;
use crate::tests::min_outbound_liquidity_channel_creator;
use bitcoin::Amount;

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn multi_hop_payment() {
    init_tracing();

    // Arrange

    let payer = Node::start_test_app("payer").unwrap();
    let coordinator = Node::start_test_coordinator("coordinator").unwrap();
    let payee = Node::start_test_app("payee").unwrap();

    payer.connect(coordinator.info).await.unwrap();
    payee.connect(coordinator.info).await.unwrap();

    coordinator.fund(Amount::from_sat(50_000)).await.unwrap();

    let payer_outbound_liquidity_sat = 20_000;
    let coordinator_outbound_liquidity_sat =
        min_outbound_liquidity_channel_creator(&payer, payer_outbound_liquidity_sat);
    coordinator
        .open_channel(
            &payer,
            coordinator_outbound_liquidity_sat,
            payer_outbound_liquidity_sat,
        )
        .await
        .unwrap();

    coordinator.open_channel(&payee, 20_000, 0).await.unwrap();

    let payer_balance_before = payer.get_ldk_balance();
    let coordinator_balance_before = coordinator.get_ldk_balance();
    let payee_balance_before = payee.get_ldk_balance();

    payer.wallet().sync().await.unwrap();
    coordinator.wallet().sync().await.unwrap();
    payee.wallet().sync().await.unwrap();

    // Act

    let invoice_amount = 1_000;
    let invoice = payee
        .create_invoice(invoice_amount, "".to_string(), 180)
        .unwrap();

    payer.send_payment(&invoice).unwrap();

    payee
        .wait_for_payment_claimed(invoice.payment_hash())
        .await
        .unwrap();

    // Assert

    // Sync LN wallet after payment is claimed to update the balances
    payer.wallet().sync().await.unwrap();
    coordinator.wallet().sync().await.unwrap();
    payee.wallet().sync().await.unwrap();

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
