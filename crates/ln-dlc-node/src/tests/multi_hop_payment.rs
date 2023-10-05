use crate::node::InMemoryStore;
use crate::node::Node;
use crate::tests::calculate_routing_fee_msat;
use crate::tests::init_tracing;
use crate::tests::wait_for_n_usable_channels;
use bitcoin::Amount;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn multi_hop_payment() {
    init_tracing();

    // Arrange

    let (payer, _running_payer) = Node::start_test_app("payer").unwrap();
    let (coordinator, _running_coord) = Node::start_test_coordinator("coordinator").unwrap();
    let (payee, _running_payee) = Node::start_test_app("payee").unwrap();

    payer.connect(coordinator.info).await.unwrap();
    payee.connect(coordinator.info).await.unwrap();

    coordinator.fund(Amount::from_sat(50_000)).await.unwrap();

    let payer_outbound_liquidity_sat = 20_000;
    let coordinator_outbound_liquidity_sat =
        min_outbound_liquidity_channel_creator(&payer, payer_outbound_liquidity_sat);
    coordinator
        .open_private_channel(
            &payer,
            coordinator_outbound_liquidity_sat,
            payer_outbound_liquidity_sat,
        )
        .await
        .unwrap();

    coordinator
        .open_private_channel(&payee, 20_000, 0)
        .await
        .unwrap();

    // after creating the just-in-time channel. The coordinator should have exactly 2 usable
    // channels with short channel ids.
    wait_for_n_usable_channels(1, &payer).await.unwrap();

    let payer_balance_before = payer.get_ldk_balance();
    let coordinator_balance_before = coordinator.get_ldk_balance();
    let payee_balance_before = payee.get_ldk_balance();

    payer.sync_on_chain().await.unwrap();
    coordinator.sync_on_chain().await.unwrap();
    payee.sync_on_chain().await.unwrap();

    // Act

    let invoice_amount_sat = 1_000;
    let invoice = payee
        .create_invoice(invoice_amount_sat, "".to_string(), 180)
        .unwrap();
    let invoice_amount_msat = invoice.amount_milli_satoshis().unwrap();

    let routing_fee_msat = calculate_routing_fee_msat(
        coordinator.ldk_config.read().channel_config,
        invoice_amount_sat,
    );

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

    assert_eq!(
        payer_balance_before.available_msat() - payer_balance_after.available_msat(),
        invoice_amount_msat + routing_fee_msat
    );

    assert_eq!(
        coordinator_balance_after.available_msat() - coordinator_balance_before.available_msat(),
        routing_fee_msat
    );

    assert_eq!(
        payee_balance_after.available_msat() - payee_balance_before.available_msat(),
        invoice_amount_msat
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
        peer.ldk_config
            .read()
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
