use crate::ln::JUST_IN_TIME_CHANNEL_OUTBOUND_LIQUIDITY_SAT;
use crate::node::Node;
use crate::node::PaymentMap;
use crate::tests::init_tracing;
use crate::tests::just_in_time_channel::TestPath;
use crate::tests::min_outbound_liquidity_channel_creator;
use crate::WalletSettings;
use anyhow::Context;
use anyhow::Result;
use bitcoin::Amount;
use lightning::chain::chaininterface::FEERATE_FLOOR_SATS_PER_KW;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use rust_decimal::RoundingStrategy;
use std::ops::Div;
use std::ops::Mul;
use std::time::Duration;

/// Based on hard-coded 1sat/vbyte fee rate in `btc-fee-estimates.json`
const CURRENT_FEE_RATE: u32 = FEERATE_FLOOR_SATS_PER_KW;

#[tokio::test(flavor = "multi_thread", worker_threads = 10)]
#[ignore]
async fn just_in_time_channel_fails_if_fee_too_low() {
    init_tracing();
    let low_fee_limit = WalletSettings {
        max_allowed_tx_fee_rate_when_opening_channel: Some(CURRENT_FEE_RATE - 1),
    };

    create_just_in_time_channel(low_fee_limit, TestPath::ExpectFundingFailure)
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 10)]
#[ignore]
async fn just_in_time_channel_works_with_correct_fee() {
    init_tracing();
    let high_fee_limit = WalletSettings {
        // We set our fee limit to above the current fee rate
        max_allowed_tx_fee_rate_when_opening_channel: Some(CURRENT_FEE_RATE + 10),
    };

    create_just_in_time_channel(high_fee_limit, TestPath::FundingAlwaysOnline)
        .await
        .unwrap();
}

async fn create_just_in_time_channel(settings: WalletSettings, test_path: TestPath) -> Result<()> {
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

    coordinator.wallet().update_settings(settings).await;

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
        test_path,
        &payer,
        &payee,
        &coordinator,
        invoice_amount,
        Some(JUST_IN_TIME_CHANNEL_OUTBOUND_LIQUIDITY_SAT),
    )
    .await
    .unwrap();
    Ok(())
}

pub(crate) async fn send_interceptable_payment(
    test_path: TestPath,
    payer: &Node<PaymentMap>,
    payee: &Node<PaymentMap>,
    coordinator: &Node<PaymentMap>,
    invoice_amount: u64,
    coordinator_just_in_time_channel_creation_outbound_liquidity: Option<u64>,
) -> Result<()> {
    payer.wallet().sync().await?;
    coordinator.wallet().sync().await?;
    payee.wallet().sync().await?;

    let payer_balance_before = payer.get_ldk_balance();
    let coordinator_balance_before = coordinator.get_ldk_balance();
    let payee_balance_before = payee.get_ldk_balance();

    // Act

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

    if TestPath::FundingThroughMobile == test_path {
        // simulate the user switching from another app to 10101

        // Note, hopefully Breez, Phoenix or any other non-custodial wallet is able to run in the
        // background when sending a payment as otherwise a disconnect on their side would happen
        // when switching to 10101 resulting into a failed payment.

        // line below is commented out on purpose
        // payer.disconnect(coordinator.info);
        tokio::time::sleep(Duration::from_secs(5)).await;
        payee.connect(coordinator.info).await?;
    }

    if let Err(e) = payee.wait_for_payment_claimed(invoice.payment_hash()).await {
        if test_path == TestPath::ExpectFundingFailure {
            // Further assertions are only relevant if the payment didn't fail
            return Ok(());
        }
        panic!("Unexpected error: {}", e);
    }

    // Assert

    // Sync LN wallet after payment is claimed to update the balances
    payer.wallet().sync().await?;
    coordinator.wallet().sync().await?;
    payee.wallet().sync().await?;

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

/// Used to ascertain if a payment will be routed through a channel
/// according to the
/// `max_inbound_htlc_value_in_flight_percent_of_channel`
/// configuration flag of the receiving end of the channel.
fn does_inbound_htlc_fit_as_percent_of_channel(
    receiving_node: &Node<PaymentMap>,
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
