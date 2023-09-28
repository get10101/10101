use crate::tests::bitcoind;
use crate::tests::init_tracing;
use crate::tests::just_in_time_channel::create::send_interceptable_payment;
use crate::tests::setup_coordinator_payer_channel;
use crate::tests::TestNode;
use std::time::Duration;

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn ln_collab_close() {
    init_tracing();

    // Arrange

    let (payer, _running_payer) = TestNode::start_test_app("payer").unwrap();
    let (coordinator, _running_coord) = TestNode::start_test_coordinator("coordinator").unwrap();
    let (payee, _running_payee) = TestNode::start_test_app("payee").unwrap();

    payer.connect(coordinator.info).await.unwrap();
    payee.connect(coordinator.info).await.unwrap();

    let payer_to_payee_invoice_amount = 10_000;
    let expected_coordinator_payee_channel_value =
        setup_coordinator_payer_channel(payer_to_payee_invoice_amount, &coordinator, &payer).await;

    send_interceptable_payment(
        &payer,
        &payee,
        &coordinator,
        payer_to_payee_invoice_amount,
        Some(expected_coordinator_payee_channel_value),
    )
    .await
    .unwrap();

    assert_eq!(payee.get_on_chain_balance().unwrap().confirmed, 0);
    assert_eq!(
        payee.get_ldk_balance().available(),
        payer_to_payee_invoice_amount
    );
    assert_eq!(payee.get_ldk_balance().pending_close(), 0);

    // Act

    let channel_id = payee
        .channel_manager
        .list_usable_channels()
        .first()
        .unwrap()
        .channel_id;

    payee
        .channel_manager
        .close_channel(&channel_id, &coordinator.info.pubkey)
        .unwrap();

    while !payee.list_channels().is_empty() {
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    // Give some time for the close transaction to be broadcast before trying to include it in a
    // block
    tokio::time::sleep(Duration::from_secs(5)).await;

    assert_eq!(payee.get_on_chain_balance().unwrap().confirmed, 0);

    // Mine one block to confirm the close transaction
    bitcoind::mine(1).await.unwrap();
    payee.sync_on_chain().await.unwrap();

    // Assert

    let ln_balance = payee.get_ldk_balance();
    assert_eq!(ln_balance.available(), 0);
    assert_eq!(ln_balance.pending_close(), 0);

    assert_eq!(
        payee.get_on_chain_balance().unwrap().confirmed,
        payer_to_payee_invoice_amount
    );
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn ln_force_close() {
    init_tracing();

    // Arrange

    let (payer, _running_payer) = TestNode::start_test_app("payer").unwrap();
    let (coordinator, _running_coord) = TestNode::start_test_coordinator("coordinator").unwrap();
    let (payee, _running_payee) = TestNode::start_test_app("payee").unwrap();

    payer.connect(coordinator.info).await.unwrap();
    payee.connect(coordinator.info).await.unwrap();

    let payer_to_payee_invoice_amount = 5_000;
    let expected_coordinator_payee_channel_value =
        setup_coordinator_payer_channel(payer_to_payee_invoice_amount, &coordinator, &payer).await;

    send_interceptable_payment(
        &payer,
        &payee,
        &coordinator,
        payer_to_payee_invoice_amount,
        Some(expected_coordinator_payee_channel_value),
    )
    .await
    .unwrap();

    assert_eq!(payee.get_on_chain_balance().unwrap().confirmed, 0);
    assert_eq!(
        payee.get_ldk_balance().available(),
        payer_to_payee_invoice_amount
    );
    assert_eq!(payee.get_ldk_balance().pending_close(), 0);

    // Act

    let channel_id = payee
        .channel_manager
        .list_usable_channels()
        .first()
        .unwrap()
        .channel_id;
    payee
        .channel_manager
        .force_close_broadcasting_latest_txn(&channel_id, &coordinator.info.pubkey)
        .unwrap();

    payee.sync_on_chain().await.unwrap();

    assert_eq!(payee.get_on_chain_balance().unwrap().confirmed, 0);
    assert_eq!(payee.get_ldk_balance().available(), 0);
    assert_eq!(
        payee.get_ldk_balance().pending_close(),
        payer_to_payee_invoice_amount
    );

    // Mine enough blocks so that the payee's revocable output in the commitment transaction
    // is spendable
    let our_to_self_delay = coordinator
        .ldk_config
        .read()
        .channel_handshake_config
        .our_to_self_delay;
    bitcoind::mine(our_to_self_delay).await.unwrap();

    // Syncing the payee's wallet should now trigger a `SpendableOutputs` event
    // corresponding to their revocable output in the commitment transaction, which they
    // will subsequently spend in a new transaction paying to their on-chain wallet
    payee.sync_on_chain().await.unwrap();

    // Mine one more block to confirm the transaction spending the payee's revocable output
    // in the commitment transaction
    bitcoind::mine(1).await.unwrap();
    payee.sync_on_chain().await.unwrap();

    // Assert

    let ln_balance = payee.get_ldk_balance();
    assert_eq!(ln_balance.available(), 0);
    assert_eq!(ln_balance.pending_close(), 0);

    let payee_txs = payee.get_on_chain_history().unwrap();

    let claim_tx = match payee_txs.as_slice() {
        [tx] => tx,
        _ => panic!(
            "Unexpected number of payee transactions. Expected 1, got {}",
            payee_txs.len()
        ),
    };

    assert_eq!(claim_tx.sent, 0);
    assert!(claim_tx.received > 0);
}
