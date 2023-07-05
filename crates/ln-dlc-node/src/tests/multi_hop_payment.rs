use crate::node::InMemoryStore;
use crate::node::Node;
use crate::tests::init_tracing;
use bitcoin::Amount;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

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

    payer.sync_on_chain().await.unwrap();
    coordinator.sync_on_chain().await.unwrap();
    payee.sync_on_chain().await.unwrap();

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
    payer.sync_on_chain().await.unwrap();
    coordinator.sync_on_chain().await.unwrap();
    payee.sync_on_chain().await.unwrap();

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

/// Calculate the "minimum" acceptable value for the outbound liquidity
/// of the channel creator.
///
/// The value calculated is not guaranteed to be the exact minimum,
/// but it should be close enough.
///
/// This is useful when the channel creator wants to push as many
/// coins as possible to their peer on channel creation.
fn min_outbound_liquidity_channel_creator(peer: &Node<InMemoryStore>, peer_balance: u64) -> u64 {
    let min_reserve_millionths_creator = Decimal::from(
        peer.user_config
            .channel_handshake_config
            .their_channel_reserve_proportional_millionths,
    );

    let min_reserve_percent_creator = min_reserve_millionths_creator / Decimal::from(1_000_000);

    // This is an approximation as we assume that `channel_balance ~=
    // peer_balance`
    let channel_balance_estimate = Decimal::from(peer_balance);

    let min_reserve_creator = min_reserve_percent_creator * channel_balance_estimate;
    let min_reserve_creator = min_reserve_creator.to_u64().unwrap();

    // The minimum reserve for any party is actually hard-coded to
    // 1_000 sats by LDK
    let min_reserve_creator = min_reserve_creator.max(1_000);

    // This is just an upper bound
    let commit_transaction_fee = 1_000;

    min_reserve_creator + commit_transaction_fee
}
