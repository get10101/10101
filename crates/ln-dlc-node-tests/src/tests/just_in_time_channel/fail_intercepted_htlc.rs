use crate::tests::init_tracing;
use crate::tests::setup_coordinator_payer_channel;
use crate::tests::TestNode;
use bitcoin::Amount;
use lightning::util::events::Event;
use ln_dlc_node::config::HTLC_INTERCEPTED_CONNECTION_TIMEOUT;
use ln_dlc_node::node::InMemoryStore;
use ln_dlc_node::node::LnDlcNodeSettings;
use ln_dlc_node::HTLCStatus;
use std::ops::Add;
use std::sync::Arc;
use std::time::Duration;

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn fail_intercepted_htlc_if_coordinator_cannot_reconnect_to_payee() {
    init_tracing();

    // Arrange

    let (payer, _running_payer) = TestNode::start_test_app("payer").unwrap();
    let (coordinator, _running_coord) = TestNode::start_test_coordinator("coordinator").unwrap();
    let (payee, _running_payee) = TestNode::start_test_app("payee").unwrap();

    payer.connect(coordinator.info).await.unwrap();
    payee.connect(coordinator.info).await.unwrap();

    let invoice_amount = 10_000;
    setup_coordinator_payer_channel(invoice_amount, &coordinator, &payer).await;

    let interceptable_route_hint_hop = coordinator
        .prepare_interceptable_payment(payee.info.pubkey)
        .unwrap();

    let invoice = payee
        .create_interceptable_invoice(
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

    payer.send_payment(&invoice).unwrap();

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

    let (payer, _running_payer) = TestNode::start_test_app("payer").unwrap();

    let (coordinator, _running_coord, mut ldk_node_event_receiver_coordinator) = {
        let (sender, receiver) = tokio::sync::watch::channel(None);
        let (coordinator, _running_coord) = TestNode::start_test_coordinator_internal(
            "coordinator",
            Arc::new(InMemoryStore::default()),
            LnDlcNodeSettings::default(),
            Some(sender),
        )
        .unwrap();

        (coordinator, _running_coord, receiver)
    };

    let (payee, _running_payee) = TestNode::start_test_app("payee").unwrap();

    payer.connect(coordinator.info).await.unwrap();
    payee.connect(coordinator.info).await.unwrap();

    let invoice_amount = 10_000;
    setup_coordinator_payer_channel(invoice_amount, &coordinator, &payer).await;

    let interceptable_route_hint_hop = coordinator
        .prepare_interceptable_payment(payee.info.pubkey)
        .unwrap();

    let invoice = payee
        .create_interceptable_invoice(
            Some(invoice_amount),
            None,
            "interceptable-invoice".to_string(),
            interceptable_route_hint_hop,
        )
        .unwrap();

    // Act

    payer.send_payment(&invoice).unwrap();

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

    let (payer, _running_payer) = TestNode::start_test_app("payer").unwrap();
    let (coordinator, _running_coord) = TestNode::start_test_coordinator("coordinator").unwrap();
    let (payee, _running_payee) = TestNode::start_test_app("payee").unwrap();

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

    let interceptable_route_hint_hop = coordinator
        .prepare_interceptable_payment(payee.info.pubkey)
        .unwrap();
    let invoice = payee
        .create_interceptable_invoice(
            Some(invoice_amount),
            None,
            "interceptable-invoice".to_string(),
            interceptable_route_hint_hop,
        )
        .unwrap();

    payer.send_payment(&invoice).unwrap();

    // Assert

    payer
        .wait_for_payment(HTLCStatus::Failed, invoice.payment_hash(), None)
        .await
        .unwrap();
}
