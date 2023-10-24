use crate::channel::UserChannelId;
use crate::config::HTLC_INTERCEPTED_CONNECTION_TIMEOUT;
use crate::node::InMemoryStore;
use crate::node::LiquidityRequest;
use crate::node::LnDlcNodeSettings;
use crate::node::Node;
use crate::tests::init_tracing;
use crate::tests::setup_coordinator_payer_channel;
use crate::HTLCStatus;
use bitcoin::Amount;
use lightning::events::Event;
use std::ops::Add;
use std::sync::Arc;
use std::time::Duration;

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn fail_intercepted_htlc_if_coordinator_cannot_reconnect_to_payee() {
    init_tracing();

    // Arrange

    let (payer, _running_payer) = Node::start_test_app("payer").unwrap();
    let (coordinator, _running_coord) = Node::start_test_coordinator("coordinator").unwrap();
    let (payee, _running_payee) = Node::start_test_app("payee").unwrap();

    payer.connect(coordinator.info).await.unwrap();
    payee.connect(coordinator.info).await.unwrap();

    let invoice_amount = 10_000;
    setup_coordinator_payer_channel(invoice_amount, &coordinator, &payer).await;

    let liquidity_request = LiquidityRequest {
        user_channel_id: UserChannelId::new(),
        liquidity_option_id: 1,
        trader_id: payee.info.pubkey,
        trade_up_to_sats: 100_000,
        max_deposit_sats: 100_000,
        coordinator_leverage: 1.0,
    };
    let interceptable_route_hint_hop = coordinator
        .prepare_onboarding_payment(liquidity_request)
        .unwrap();

    let invoice = payee
        .create_invoice_with_route_hint(
            Some(invoice_amount),
            None,
            "interceptable-invoice".to_string(),
            interceptable_route_hint_hop,
        )
        .unwrap();

    // Act

    // We wait a second for payee and coordinator to be disconnected
    payee.disconnect(coordinator.info);
    tokio::time::sleep(Duration::from_secs(1)).await;

    payer.pay_invoice(&invoice, None).unwrap();

    // Assert

    payer
        .wait_for_payment(
            HTLCStatus::Failed,
            invoice.payment_hash(),
            // We wait a bit longer than what the coordinator should wait for the payee to
            // reconnect
            Some(HTLC_INTERCEPTED_CONNECTION_TIMEOUT.add(Duration::from_secs(5))),
        )
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn fail_intercepted_htlc_if_connection_lost_after_funding_tx_generated() {
    init_tracing();

    // Arrange

    let (payer, _running_payer) = Node::start_test_app("payer").unwrap();

    let (coordinator, _running_coord, mut ldk_node_event_receiver_coordinator) = {
        let (sender, receiver) = tokio::sync::watch::channel(None);
        let (coordinator, _running_coord) = Node::start_test_coordinator_internal(
            "coordinator",
            Arc::new(InMemoryStore::default()),
            LnDlcNodeSettings::default(),
            Some(sender),
        )
        .unwrap();

        (coordinator, _running_coord, receiver)
    };

    let (payee, _running_payee) = Node::start_test_app("payee").unwrap();

    payer.connect(coordinator.info).await.unwrap();
    payee.connect(coordinator.info).await.unwrap();

    let invoice_amount = 10_000;
    setup_coordinator_payer_channel(invoice_amount, &coordinator, &payer).await;

    let liquidity_request = LiquidityRequest {
        user_channel_id: UserChannelId::new(),
        liquidity_option_id: 1,
        trader_id: payee.info.pubkey,
        trade_up_to_sats: 100_000,
        max_deposit_sats: 100_000,
        coordinator_leverage: 1.0,
    };
    let interceptable_route_hint_hop = coordinator
        .prepare_onboarding_payment(liquidity_request)
        .unwrap();

    let invoice = payee
        .create_invoice_with_route_hint(
            Some(invoice_amount),
            None,
            "interceptable-invoice".to_string(),
            interceptable_route_hint_hop,
        )
        .unwrap();

    // Act

    payer.pay_invoice(&invoice, None).unwrap();

    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            ldk_node_event_receiver_coordinator.changed().await.unwrap();
            let event = ldk_node_event_receiver_coordinator.borrow().clone();

            if let Some(Event::FundingGenerationReady { .. }) = event {
                // We wait a second for payee and coordinator to be disconnected
                payee.disconnect(coordinator.info);
                tokio::time::sleep(Duration::from_secs(1)).await;

                break;
            }
        }
    })
    .await
    .unwrap();

    // Assert

    payer
        .wait_for_payment(HTLCStatus::Failed, invoice.payment_hash(), None)
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn fail_intercepted_htlc_if_coordinator_cannot_pay_to_open_jit_channel() {
    init_tracing();

    // Arrange

    let (payer, _running_payer) = Node::start_test_app("payer").unwrap();
    let (coordinator, _running_coord) = Node::start_test_coordinator("coordinator").unwrap();
    let (payee, _running_payee) = Node::start_test_app("payee").unwrap();

    payer.connect(coordinator.info).await.unwrap();
    payee.connect(coordinator.info).await.unwrap();

    let payer_outbound_liquidity = 200_000;

    payer.fund(Amount::ONE_BTC).await.unwrap();
    payer
        .open_public_channel(&coordinator, payer_outbound_liquidity, 0)
        .await
        .unwrap();

    // Act

    // The coordinator should not be able to open any JIT channel because we have not funded their
    // on-chain wallet
    let invoice_amount = 10_000;

    let liquidity_request = LiquidityRequest {
        user_channel_id: UserChannelId::new(),
        liquidity_option_id: 1,
        trader_id: payee.info.pubkey,
        trade_up_to_sats: 100_000,
        max_deposit_sats: 100_000,
        coordinator_leverage: 1.0,
    };
    let interceptable_route_hint_hop = coordinator
        .prepare_onboarding_payment(liquidity_request)
        .unwrap();
    let invoice = payee
        .create_invoice_with_route_hint(
            Some(invoice_amount),
            None,
            "interceptable-invoice".to_string(),
            interceptable_route_hint_hop,
        )
        .unwrap();

    payer.pay_invoice(&invoice, None).unwrap();

    // Assert

    payer
        .wait_for_payment(HTLCStatus::Failed, invoice.payment_hash(), None)
        .await
        .unwrap();
}
