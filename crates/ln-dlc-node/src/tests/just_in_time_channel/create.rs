use crate::channel::Channel;
use crate::channel::ChannelState;
use crate::channel::UserChannelId;
use crate::fee_rate_estimator::EstimateFeeRate;
use crate::node::InMemoryStore;
use crate::node::LiquidityRequest;
use crate::node::LnDlcNodeSettings;
use crate::node::Node;
use crate::node::Storage;
use crate::storage::TenTenOneInMemoryStorage;
use crate::tests::calculate_routing_fee_msat;
use crate::tests::init_tracing;
use crate::tests::ln_dlc_node_settings_coordinator;
use crate::tests::setup_coordinator_payer_channel;
use crate::HTLCStatus;
use crate::WalletSettings;
use anyhow::Context;
use anyhow::Result;
use lightning::chain::chaininterface::ConfirmationTarget;
use lightning::ln::ChannelId;
use rust_decimal::Decimal;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn open_jit_channel() {
    init_tracing();

    // Arrange

    let (payer, _running_payer) = Node::start_test_app("payer").unwrap();

    let coordinator_storage = Arc::new(InMemoryStore::default());
    let settings = LnDlcNodeSettings {
        on_chain_sync_interval: Duration::from_secs(3),
        shadow_sync_interval: Duration::from_secs(3),
        ..ln_dlc_node_settings_coordinator()
    };
    let (coordinator, _running_coordinator) = {
        // setting the on chain sync interval to 5 seconds so that we don't have to wait for so long
        // before the costs for the funding transaction will be attached to the shadow channel.

        Node::start_test_coordinator_internal(
            "coordinator",
            coordinator_storage.clone(),
            settings.clone(),
            None,
        )
        .unwrap()
    };
    let (payee, _running_payee) = Node::start_test_app("payee").unwrap();

    payer.connect(coordinator.info).await.unwrap();
    payee.connect(coordinator.info).await.unwrap();

    // Testing with a large invoice amount.
    let payer_to_payee_invoice_amount_sat = 3_000_000;
    let (expected_coordinator_payee_channel_value, liquidity_request) =
        setup_coordinator_payer_channel(
            &coordinator,
            &payer,
            payee.info.pubkey,
            payer_to_payee_invoice_amount_sat,
        )
        .await;

    // Act and assert
    send_interceptable_payment(
        &payer,
        &payee,
        &coordinator,
        payer_to_payee_invoice_amount_sat,
        liquidity_request,
        expected_coordinator_payee_channel_value,
    )
    .await
    .unwrap();

    let channel_details = coordinator
        .channel_manager
        .list_usable_channels()
        .iter()
        .find(|c| c.counterparty.node_id == payee.info.pubkey)
        .context("Could not find usable channel with peer")
        .unwrap()
        .clone();

    let user_channel_id = Uuid::from_u128(channel_details.user_channel_id).to_string();

    // Wait for costs getting attached to the shadow channel. We are waiting for 6 seconds to ensure
    // that the shadow sync will run at least once.
    tokio::time::sleep(Duration::from_secs(6)).await;

    let channel = coordinator_storage
        .get_channel(&user_channel_id)
        .unwrap()
        .unwrap();
    assert_eq!(ChannelState::OpenUnpaid, channel.channel_state);

    let transaction = coordinator_storage
        .get_transaction(&channel.funding_txid.unwrap().to_string())
        .unwrap()
        .unwrap();
    assert!(transaction.fee() > 0);
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn fail_to_open_jit_channel_with_fee_rate_over_max() {
    init_tracing();

    // Arrange

    let (payer, _running_payer) = Node::start_test_app("payer").unwrap();
    let (coordinator, _running_coord) = Node::start_test_coordinator("coordinator").unwrap();
    let (payee, _running_payee) = Node::start_test_app("payee").unwrap();

    payer.connect(coordinator.info).await.unwrap();
    payee.connect(coordinator.info).await.unwrap();

    let payer_to_payee_invoice_amount = 5_000;
    let _ = setup_coordinator_payer_channel(
        &coordinator,
        &payer,
        payee.info.pubkey,
        payer_to_payee_invoice_amount,
    )
    .await;

    // Act

    let background_fee_rate = coordinator
        .fee_rate_estimator
        .estimate(ConfirmationTarget::Background)
        .fee_wu(1000) as u32;

    // Set max allowed TX fee rate when opening channel to a value below the current background fee
    // rate to ensure that opening the JIT channel fails
    let settings = WalletSettings {
        max_allowed_tx_fee_rate_when_opening_channel: Some(background_fee_rate - 1),
        jit_channels_enabled: true,
    };

    coordinator.ldk_wallet().update_settings(settings).await;

    let liquidity_request = LiquidityRequest {
        user_channel_id: UserChannelId::new(),
        liquidity_option_id: 1,
        trader_id: payee.info.pubkey,
        trade_up_to_sats: 200_000,
        max_deposit_sats: 200_000,
        coordinator_leverage: 1.0,
        fee_sats: 10_000,
    };
    let final_route_hint_hop = coordinator
        .prepare_onboarding_payment(liquidity_request)
        .unwrap();
    let invoice = payee
        .create_invoice_with_route_hint(
            Some(payer_to_payee_invoice_amount),
            None,
            "interceptable-invoice".to_string(),
            final_route_hint_hop,
        )
        .unwrap();

    payer.pay_invoice(&invoice, None).unwrap();

    // Assert

    // We would like to assert on the payment failing, but this is not guaranteed as the payment can
    // still be retried after the first payment path failure. Thus, we check that it doesn't succeed
    payee
        .wait_for_payment(HTLCStatus::Succeeded, invoice.payment_hash(), None)
        .await
        .expect_err("payment should not succeed");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn open_jit_channel_with_disconnected_payee() {
    init_tracing();

    // Arrange

    let (payer, _running_payer) = Node::start_test_app("payer").unwrap();
    let (coordinator, _running_coord) = Node::start_test_coordinator("coordinator").unwrap();
    let (payee, _running_payee) = Node::start_test_app("payee").unwrap();

    // We purposefully do NOT connect to the payee, so that we can test the ability to open a JIT
    // channel to a disconnected payee
    payer.connect(coordinator.info).await.unwrap();

    let payer_to_payee_invoice_amount = 5_000;
    let _ = setup_coordinator_payer_channel(
        &coordinator,
        &payer,
        payee.info.pubkey,
        payer_to_payee_invoice_amount,
    )
    .await;

    // Act

    let liquidity_request = LiquidityRequest {
        user_channel_id: UserChannelId::new(),
        liquidity_option_id: 1,
        trader_id: payee.info.pubkey,
        trade_up_to_sats: payer_to_payee_invoice_amount,
        max_deposit_sats: payer_to_payee_invoice_amount,
        coordinator_leverage: 1.0,
        fee_sats: 0,
    };
    let final_route_hint_hop = coordinator
        .prepare_onboarding_payment(liquidity_request)
        .unwrap();

    let invoice = payee
        .create_invoice_with_route_hint(
            Some(payer_to_payee_invoice_amount),
            None,
            "interceptable-invoice".to_string(),
            final_route_hint_hop,
        )
        .unwrap();

    payer.pay_invoice(&invoice, None).unwrap();

    // We wait a little bit until reconnecting to simulate a pending JIT channel on the coordinator
    tokio::time::sleep(Duration::from_secs(5)).await;
    payee.connect(coordinator.info).await.unwrap();

    // Assert

    payee
        .wait_for_payment_claimed(invoice.payment_hash())
        .await
        .unwrap();
}

pub(crate) async fn send_interceptable_payment(
    payer: &Node<TenTenOneInMemoryStorage, InMemoryStore>,
    payee: &Node<TenTenOneInMemoryStorage, InMemoryStore>,
    coordinator: &Node<TenTenOneInMemoryStorage, InMemoryStore>,
    invoice_amount_sat: u64,
    liquidity_request: LiquidityRequest,
    expected_coordinator_payee_channel_value_sat: u64,
) -> Result<()> {
    let user_channel_id = liquidity_request.user_channel_id;
    let jit_channel_fee = liquidity_request.fee_sats;

    payer.sync_wallets().await?;
    coordinator.sync_wallets().await?;
    payee.sync_wallets().await?;

    let payer_balance_before = payer.get_ldk_balance();
    let coordinator_balance_before = coordinator.get_ldk_balance();
    let payee_balance_before = payee.get_ldk_balance();

    let interceptable_route_hint_hop = coordinator.prepare_onboarding_payment(liquidity_request)?;

    // Announce the jit channel on the app side. This is done by the app when preparing the
    // onboarding invoice. But since we do not have the app available here, we need to do this
    // manually.
    let channel =
        Channel::new_jit_channel(user_channel_id, coordinator.info.pubkey, 1, jit_channel_fee);
    payee
        .node_storage
        .upsert_channel(channel)
        .with_context(|| {
            format!("Failed to insert shadow JIT channel with user channel id {user_channel_id}")
        })?;

    let invoice = payee.create_invoice_with_route_hint(
        Some(invoice_amount_sat),
        None,
        "interceptable-invoice".to_string(),
        interceptable_route_hint_hop,
    )?;
    let invoice_amount_msat = invoice.amount_milli_satoshis().unwrap();

    let routing_fee_msat = calculate_routing_fee_msat(
        coordinator.ldk_config.read().channel_config,
        invoice_amount_sat,
    );

    assert!(
        does_inbound_htlc_fit_as_percent_of_channel(
            coordinator,
            &payer
                .channel_manager
                .list_channels()
                .first()
                .expect("payer channel should be created.")
                .channel_id,
            invoice_amount_sat + (routing_fee_msat / 1000)
        )
        .unwrap(),
        "Invoice amount larger than maximum inbound HTLC in payer-coordinator channel"
    );

    payer.pay_invoice(&invoice, None).unwrap();

    payee
        .wait_for_payment_claimed(invoice.payment_hash())
        .await
        .unwrap();

    // Assert

    // Sync LN wallet after payment is claimed to update the balances
    payer.sync_wallets().await?;
    coordinator.sync_wallets().await?;
    payee.sync_wallets().await?;

    let payer_balance_after = payer.get_ldk_balance();
    let coordinator_balance_after = coordinator.get_ldk_balance();
    let payee_balance_after = payee.get_ldk_balance();

    assert_eq!(
        payer_balance_before.available_msat() - payer_balance_after.available_msat(),
        invoice_amount_msat + routing_fee_msat
    );

    assert_eq!(
        coordinator_balance_after.available_msat() - coordinator_balance_before.available_msat(),
        expected_coordinator_payee_channel_value_sat * 1000
            + routing_fee_msat
            + jit_channel_fee * 1000
    );

    assert_eq!(
        payee_balance_after.available() - payee_balance_before.available(),
        invoice_amount_sat - jit_channel_fee
    );

    Ok(())
}

/// Sends a regular payment assuming all channels on the path exist.
pub(crate) async fn send_payment(
    payer: &Node<TenTenOneInMemoryStorage, InMemoryStore>,
    payee: &Node<TenTenOneInMemoryStorage, InMemoryStore>,
    coordinator: &Node<TenTenOneInMemoryStorage, InMemoryStore>,
    invoice_amount_sat: u64,
    coordinator_just_in_time_channel_creation_outbound_liquidity: Option<u64>,
) -> Result<()> {
    payer.sync_wallets().await?;
    coordinator.sync_wallets().await?;
    payee.sync_wallets().await?;

    let payer_balance_before = payer.get_ldk_balance();
    let coordinator_balance_before = coordinator.get_ldk_balance();
    let payee_balance_before = payee.get_ldk_balance();

    let route_hint_hop = payee.prepare_payment_with_route_hint(coordinator.info.pubkey)?;

    let invoice = payee.create_invoice_with_route_hint(
        Some(invoice_amount_sat),
        None,
        "regular invoice".to_string(),
        route_hint_hop,
    )?;
    let invoice_amount_msat = invoice.amount_milli_satoshis().unwrap();

    let routing_fee_msat = calculate_routing_fee_msat(
        coordinator.ldk_config.read().channel_config,
        invoice_amount_sat,
    );

    assert!(
        does_inbound_htlc_fit_as_percent_of_channel(
            coordinator,
            &payer
                .channel_manager
                .list_channels()
                .first()
                .expect("payer channel should be created.")
                .channel_id,
            invoice_amount_sat + (routing_fee_msat / 1000)
        )
        .unwrap(),
        "Invoice amount larger than maximum inbound HTLC in payer-coordinator channel"
    );

    payer.pay_invoice(&invoice, None).unwrap();

    payee
        .wait_for_payment_claimed(invoice.payment_hash())
        .await
        .unwrap();

    // Assert

    // Sync LN wallet after payment is claimed to update the balances
    payer.sync_wallets().await?;
    coordinator.sync_wallets().await?;
    payee.sync_wallets().await?;

    let payer_balance_after = payer.get_ldk_balance();
    let coordinator_balance_after = coordinator.get_ldk_balance();
    let payee_balance_after = payee.get_ldk_balance();

    assert_eq!(
        payer_balance_before.available_msat() - payer_balance_after.available_msat(),
        invoice_amount_msat + routing_fee_msat
    );

    assert_eq!(
        coordinator_balance_after.available_msat() - coordinator_balance_before.available_msat(),
        coordinator_just_in_time_channel_creation_outbound_liquidity.unwrap_or_default() * 1000
            + routing_fee_msat
    );

    assert_eq!(
        payee_balance_after.available() - payee_balance_before.available(),
        invoice_amount_sat
    );

    Ok(())
}

/// Used to ascertain if a payment will be routed through a channel according to the
/// `max_inbound_htlc_value_in_flight_percent_of_channel` configuration flag of the receiving end of
/// the channel.
fn does_inbound_htlc_fit_as_percent_of_channel(
    receiving_node: &Node<TenTenOneInMemoryStorage, InMemoryStore>,
    channel_id: &ChannelId,
    htlc_amount_sat: u64,
) -> Result<bool> {
    let htlc_amount_sat = Decimal::from(htlc_amount_sat);

    let max_inbound_htlc_as_percent_of_channel = Decimal::from(
        receiving_node
            .ldk_config
            .read()
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
