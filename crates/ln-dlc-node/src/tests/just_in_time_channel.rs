use crate::ln::event_handler::JUST_IN_TIME_CHANNEL_OUTBOUND_LIQUIDITY_SAT;
use crate::node::Node;
use crate::node::LIQUIDITY_ROUTING_FEE_MILLIONTHS;
use crate::tests::init_tracing;
use anyhow::Context;
use anyhow::Result;
use bitcoin::Amount;
use rust_decimal::Decimal;
use std::time::Duration;

#[tokio::test]
#[ignore]
async fn just_in_time_channel() {
    init_tracing();

    // Arrange

    let payer = Node::start_test_app("payer").await.unwrap();
    let coordinator = Node::start_test_coordinator("coordinator").await.unwrap();
    let payee = Node::start_test_app("payee").await.unwrap();

    payer.keep_connected(coordinator.info).await.unwrap();
    payee.keep_connected(coordinator.info).await.unwrap();

    // Fund the on-chain wallets of the nodes who will open a channel
    payer.fund(Amount::from_sat(100_000)).await.unwrap();
    coordinator.fund(Amount::from_sat(100_000)).await.unwrap();

    let coordinator_outbound_liquidity_sat = 25_000;
    let payer_outbound_liquidity_sat = 25_000;
    let payer_coordinator_channel_details = coordinator
        .open_channel(
            &payer,
            coordinator_outbound_liquidity_sat,
            payer_outbound_liquidity_sat,
        )
        .await
        .unwrap();

    let payer_balance_before = payer.get_ldk_balance();
    let coordinator_balance_before = coordinator.get_ldk_balance();
    let payee_balance_before = payee.get_ldk_balance();

    // Act

    let intercept_scid = coordinator.create_intercept_scid(payee.info.pubkey);

    // This comment should be removed once the implementation of
    // `Node` is improved.
    //
    // What values don't work and why:
    //
    // Obviously, this amount must be smaller than or equal to the
    // outbound liquidity of the payer in the payer-coordinator
    // channel. Similarly, it must be smaller than or equal to the
    // outbound liquidity of the coordinator in the just-in-time
    // channel between coordinator and payee.
    //
    // But there's more. This value (plus fees) must be a smaller than
    // or equal percentage of the payer-coordinator channel than the
    // coordinator's
    // `max_inbound_htlc_value_in_flight_percent_of_channel`
    // configuration value for said channel. That is checked by the
    // assertion below.
    //
    // But there is still more! This value must be a smaller than or
    // equal percentage of the just-in-time coordinator-payee channel
    // than the payee's
    // `max_inbound_htlc_value_in_flight_percent_of_channel`
    // configuration value for said channel.
    //
    // At this point in time, we fix the channel size to
    // `JUST_IN_TIME_CHANNEL_OUTBOUND_LIQUIDITY_SAT` (10_000 sats) and
    // the default value of
    // `max_inbound_htlc_value_in_flight_percent_of_channel` is set to
    // 10 percent. This means that we won't succeed in routing the
    // payment through the just-in-time channel if the amount is
    // greater than 1_000 sats.
    let invoice_amount = 1_000;

    let flat_routing_fee = 1; // according to the default `ChannelConfig`
    let liquidity_routing_fee =
        (invoice_amount * LIQUIDITY_ROUTING_FEE_MILLIONTHS as u64) / 1_000_000;

    assert!(
        does_inbound_htlc_fit_as_percent_of_channel(
            &coordinator,
            &payer_coordinator_channel_details.channel_id,
            invoice_amount + flat_routing_fee + liquidity_routing_fee
        )
        .unwrap(),
        "Invoice amount larger than maximum inbound HTLC in payer-coordinator channel"
    );

    let invoice_expiry = 0; // an expiry of 0 means the invoice never expires
    let invoice = payee
        .create_interceptable_invoice(
            invoice_amount,
            intercept_scid,
            coordinator.info.pubkey,
            invoice_expiry,
            "interceptable-invoice".to_string(),
        )
        .unwrap();

    payer.send_payment(&invoice).unwrap();

    // For the payment to be claimed before the wallets sync
    tokio::time::sleep(Duration::from_secs(3)).await;

    payer.sync();
    coordinator.sync();
    payee.sync();

    // Assert

    let payer_balance_after = payer.get_ldk_balance();
    let coordinator_balance_after = coordinator.get_ldk_balance();
    let payee_balance_after = payee.get_ldk_balance();

    assert_eq!(
        payer_balance_before.available - payer_balance_after.available,
        invoice_amount + flat_routing_fee + liquidity_routing_fee
    );

    assert_eq!(
        coordinator_balance_after.available - coordinator_balance_before.available,
        JUST_IN_TIME_CHANNEL_OUTBOUND_LIQUIDITY_SAT + flat_routing_fee + liquidity_routing_fee
    );

    assert_eq!(
        payee_balance_after.available - payee_balance_before.available,
        invoice_amount
    );
}

/// Used to ascertain if a payment will be routed through a channel
/// according to the
/// `max_inbound_htlc_value_in_flight_percent_of_channel`
/// configuration flag of the receiving end of the channel.
fn does_inbound_htlc_fit_as_percent_of_channel(
    receiving_node: &Node,
    channel_id: &[u8; 32],
    htlc_amount_sat: u64,
) -> Result<bool> {
    let htlc_amount_sat = Decimal::from(htlc_amount_sat);

    let max_inbound_htlc_as_percent_of_channel = Decimal::from(
        receiving_node
            .user_config
            .channel_handshake_config
            .max_inbound_htlc_value_in_flight_percent_of_channel,
    );

    let channel_size_sat = receiving_node
        .channel_manager
        .list_channels()
        .iter()
        .find_map(|c| (&c.channel_id == channel_id).then_some(c.channel_value_satoshis))
        .context("No matching channel")?;
    let channel_size_sat = Decimal::from(channel_size_sat);

    let max_inbound_htlc_sat =
        channel_size_sat * (max_inbound_htlc_as_percent_of_channel / Decimal::ONE_HUNDRED);

    Ok(htlc_amount_sat <= max_inbound_htlc_sat)
}
