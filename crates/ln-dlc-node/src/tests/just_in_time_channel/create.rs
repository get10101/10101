use crate::ln::JUST_IN_TIME_CHANNEL_OUTBOUND_LIQUIDITY_SAT;
use crate::node::InMemoryStore;
use crate::node::Node;
use crate::tests::init_tracing;
use crate::tests::min_outbound_liquidity_channel_creator;
use crate::HTLCStatus;
use crate::WalletSettings;
use anyhow::Context;
use anyhow::Result;
use bitcoin::Amount;
use lightning::chain::chaininterface::ConfirmationTarget;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use rust_decimal::RoundingStrategy;
use std::ops::Div;
use std::ops::Mul;
use std::time::Duration;

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn open_jit_channel() {
    init_tracing();

    // Arrange

    let payer = Node::start_test_app("payer").unwrap();
    let coordinator = Node::start_test_coordinator("coordinator").unwrap();
    let payee = Node::start_test_app("payee").unwrap();

    payer.connect(coordinator.info).await.unwrap();
    payee.connect(coordinator.info).await.unwrap();

    coordinator.fund(Amount::from_sat(1_000_000)).await.unwrap();

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

    // Act and assert

    send_interceptable_payment(
        &payer,
        &payee,
        &coordinator,
        1_000,
        Some(JUST_IN_TIME_CHANNEL_OUTBOUND_LIQUIDITY_SAT),
    )
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn fail_to_open_jit_channel_with_fee_rate_over_max() {
    init_tracing();

    // Arrange

    let payer = Node::start_test_app("payer").unwrap();
    let coordinator = Node::start_test_coordinator("coordinator").unwrap();
    let payee = Node::start_test_app("payee").unwrap();

    payer.connect(coordinator.info).await.unwrap();
    payee.connect(coordinator.info).await.unwrap();

    coordinator.fund(Amount::from_sat(1_000_000)).await.unwrap();

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

    // Act

    let background_fee_rate = coordinator
        .fee_rate_estimator
        .get(ConfirmationTarget::Background)
        .fee_wu(1000) as u32;

    // Set max allowed TX fee rate when opening channel to a value below the current background fee
    // rate to ensure that opening the JIT channel fails
    let settings = WalletSettings {
        max_allowed_tx_fee_rate_when_opening_channel: Some(background_fee_rate - 1),
    };

    coordinator.wallet().update_settings(settings).await;

    let intercepted_scid_details = coordinator.create_intercept_scid(payee.info.pubkey, 50);

    let invoice = payee
        .create_interceptable_invoice(
            Some(1_000),
            intercepted_scid_details.scid,
            coordinator.info.pubkey,
            0,
            "interceptable-invoice".to_string(),
            intercepted_scid_details.jit_routing_fee_millionth,
        )
        .unwrap();

    payer.send_payment(&invoice).unwrap();

    // Assert

    // We would like to assert on the payment failing, but this is not guaranteed as the payment can
    // still be retried after the first payment path failure. Thus, we check that it doesn't succeed
    payee
        .wait_for_payment(HTLCStatus::Succeeded, invoice.payment_hash())
        .await
        .expect_err("payment should not succeed");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn open_jit_channel_with_disconnected_payee() {
    init_tracing();

    // Arrange

    let payer = Node::start_test_app("payer").unwrap();
    let coordinator = Node::start_test_coordinator("coordinator").unwrap();
    let payee = Node::start_test_app("payee").unwrap();

    // We purposefully do NOT connect to the payee, so that we can test the ability to open a JIT
    // channel to a disconnected payee
    payer.connect(coordinator.info).await.unwrap();

    coordinator.fund(Amount::from_sat(1_000_000)).await.unwrap();

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

    // Act

    let intercepted_scid_details = coordinator.create_intercept_scid(payee.info.pubkey, 50);

    let invoice = payee
        .create_interceptable_invoice(
            Some(1_000),
            intercepted_scid_details.scid,
            coordinator.info.pubkey,
            0,
            "interceptable-invoice".to_string(),
            intercepted_scid_details.jit_routing_fee_millionth,
        )
        .unwrap();

    payer.send_payment(&invoice).unwrap();

    // We wait a little bit until reconnecting to simulate a pending JIT channel on the coordinator
    tokio::time::sleep(Duration::from_secs(5)).await;
    payee.connect(coordinator.info).await.unwrap();

    // Assert

    payee
        .wait_for_payment_claimed(invoice.payment_hash())
        .await
        .unwrap();
}

/// The caller should ensure that the `invoice_amount` is:
///
/// - Smaller than or equal to the outbound liquidity of the payer in the payer-coordinator channel.
///
/// - Smaller than or equal to the outbound liquidity of the coordinator in the coordinator-payee
/// JIT channel.
///
/// - Smaller than or equal proportion of the payee's
/// `max_inbound_htlc_value_in_flight_percent_of_channel` configuration value for for the
/// coordinator-payee JIT channel.
///
/// Additionally, the `invoice_amount` (plus routing fees) should be a proportion of the value of
/// the payer-coordinator channel no larger than the coordinator's
/// `max_inbound_htlc_value_in_flight_percent_of_channel` configuration value for said channel. This
/// is verified within this function.
pub(crate) async fn send_interceptable_payment(
    payer: &Node<InMemoryStore>,
    payee: &Node<InMemoryStore>,
    coordinator: &Node<InMemoryStore>,
    invoice_amount: u64,
    coordinator_just_in_time_channel_creation_outbound_liquidity: Option<u64>,
) -> Result<()> {
    payer.wallet().sync()?;
    coordinator.wallet().sync()?;
    payee.wallet().sync()?;

    let payer_balance_before = payer.get_ldk_balance();
    let coordinator_balance_before = coordinator.get_ldk_balance();
    let payee_balance_before = payee.get_ldk_balance();

    let jit_fee = 50;
    let intercepted_scid_details = coordinator.create_intercept_scid(payee.info.pubkey, jit_fee);
    let intercept_scid = intercepted_scid_details.scid;
    let fee_millionth = intercepted_scid_details.jit_routing_fee_millionth;

    let flat_routing_fee = 1; // according to the default `ChannelConfig`
    let liquidity_routing_fee = Decimal::from_u64(invoice_amount)
        .unwrap()
        .mul(Decimal::from_u32(fee_millionth).unwrap())
        .div(Decimal::from_u64(1_000_000).unwrap());
    let liquidity_routing_fee_payer = liquidity_routing_fee
        .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
        .to_u64()
        .unwrap();
    let liquidity_routing_fee_receiver = liquidity_routing_fee
        .round_dp_with_strategy(0, RoundingStrategy::MidpointTowardZero)
        .to_u64()
        .unwrap();

    assert!(
        does_inbound_htlc_fit_as_percent_of_channel(
            coordinator,
            &payer
                .channel_manager
                .list_channels()
                .first()
                .expect("payer channel should be created.")
                .channel_id,
            invoice_amount + flat_routing_fee + liquidity_routing_fee_receiver
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
        fee_millionth,
    )?;

    payer.send_payment(&invoice)?;

    payee
        .wait_for_payment_claimed(invoice.payment_hash())
        .await?;

    // Assert

    // Sync LN wallet after payment is claimed to update the balances
    payer.wallet().sync()?;
    coordinator.wallet().sync()?;
    payee.wallet().sync()?;

    let payer_balance_after = payer.get_ldk_balance();
    let coordinator_balance_after = coordinator.get_ldk_balance();
    let payee_balance_after = payee.get_ldk_balance();

    assert_eq!(
        payer_balance_before.available - payer_balance_after.available,
        invoice_amount + flat_routing_fee + liquidity_routing_fee_payer
    );

    assert_eq!(
        coordinator_balance_after.available - coordinator_balance_before.available,
        coordinator_just_in_time_channel_creation_outbound_liquidity.unwrap_or_default()
            + flat_routing_fee
            + liquidity_routing_fee_receiver
    );

    assert_eq!(
        payee_balance_after.available - payee_balance_before.available,
        invoice_amount
    );

    Ok(())
}

/// Used to ascertain if a payment will be routed through a channel according to the
/// `max_inbound_htlc_value_in_flight_percent_of_channel` configuration flag of the receiving end of
/// the channel.
fn does_inbound_htlc_fit_as_percent_of_channel(
    receiving_node: &Node<InMemoryStore>,
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
