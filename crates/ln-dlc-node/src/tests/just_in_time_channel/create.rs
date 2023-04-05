use crate::ln::JUST_IN_TIME_CHANNEL_OUTBOUND_LIQUIDITY_SAT;
use crate::node::Node;
use crate::node::LIQUIDITY_ROUTING_FEE_MILLIONTHS;
use crate::tests::init_tracing;
use crate::tests::min_outbound_liquidity_channel_creator;
use anyhow::Context;
use anyhow::Result;
use bitcoin::Amount;
use rust_decimal::Decimal;

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

    coordinator.fund(Amount::from_sat(100_000)).await.unwrap();

    let payer_outbound_liquidity_sat = 25_000;
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
    let invoice_amount = 1_000;

    send_interceptable_payment(
        &payer,
        &payee,
        &coordinator,
        invoice_amount,
        Some(JUST_IN_TIME_CHANNEL_OUTBOUND_LIQUIDITY_SAT),
    )
    .await
    .unwrap();
}

pub(crate) async fn send_interceptable_payment(
    payer: &Node,
    payee: &Node,
    coordinator: &Node,
    invoice_amount: u64,
    coordinator_just_in_time_channel_creation_outbound_liquidity: Option<u64>,
) -> Result<()> {
    payer.sync()?;
    coordinator.sync()?;
    payee.sync()?;

    let payer_balance_before = payer.get_ldk_balance();
    let coordinator_balance_before = coordinator.get_ldk_balance();
    let payee_balance_before = payee.get_ldk_balance();

    // Act

    let intercept_scid = coordinator.create_intercept_scid(payee.info.pubkey);

    let flat_routing_fee = 1; // according to the default `ChannelConfig`
    let liquidity_routing_fee =
        (invoice_amount * LIQUIDITY_ROUTING_FEE_MILLIONTHS as u64) / 1_000_000;

    assert!(
        does_inbound_htlc_fit_as_percent_of_channel(
            coordinator,
            &payer
                .channel_manager
                .list_channels()
                .first()
                .expect("payer channel should be created.")
                .channel_id,
            invoice_amount + flat_routing_fee + liquidity_routing_fee
        )
        .unwrap(),
        "Invoice amount larger than maximum inbound HTLC in payer-coordinator channel"
    );

    let invoice_expiry = 0; // an expiry of 0 means the invoice never expires
    let invoice = payee.create_interceptable_invoice(
        Some(invoice_amount),
        intercept_scid,
        coordinator.info.pubkey,
        invoice_expiry,
        "interceptable-invoice".to_string(),
    )?;

    payer.send_payment(&invoice)?;

    payee
        .wait_for_payment_claimed(invoice.payment_hash())
        .await?;

    // Assert

    // Sync LN wallet after payment is claimed to update the balances
    payer.sync()?;
    coordinator.sync()?;
    payee.sync()?;

    let payer_balance_after = payer.get_ldk_balance();
    let coordinator_balance_after = coordinator.get_ldk_balance();
    let payee_balance_after = payee.get_ldk_balance();

    assert_eq!(
        payer_balance_before.available - payer_balance_after.available,
        invoice_amount + flat_routing_fee + liquidity_routing_fee
    );

    assert_eq!(
        coordinator_balance_after.available - coordinator_balance_before.available,
        coordinator_just_in_time_channel_creation_outbound_liquidity.unwrap_or_default()
            + flat_routing_fee
            + liquidity_routing_fee
    );

    assert_eq!(
        payee_balance_after.available - payee_balance_before.available,
        invoice_amount
    );

    Ok(())
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
