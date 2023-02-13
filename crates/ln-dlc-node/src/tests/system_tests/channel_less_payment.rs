use bitcoin::Address;
use std::str::FromStr;
use std::time::Duration;
// Related issue: https://github.com/get10101/10101/issues/55.
use crate::tests::system_tests::create_tmp_dir;
use crate::tests::system_tests::fund_and_mine;
use crate::tests::system_tests::has_channel;
use crate::tests::system_tests::init_tracing;
use crate::tests::system_tests::log_channel_id;
use crate::tests::system_tests::setup_ln_node;

#[tokio::test]
async fn given_no_channel_with_coordinator_when_invoice_generated_then_can_be_paid_through_coordinator(
) {
    init_tracing();

    let test_dir = create_tmp_dir("channel_less_payment");

    // 1. Set up three LN-DLC nodes.
    let alice = setup_ln_node(&test_dir, "alice", false).await;
    let coordinator = setup_ln_node(&test_dir, "coordinator", true).await;
    let bob = setup_ln_node(&test_dir, "bob", false).await;

    tracing::info!("alice: {}", alice.info);
    tracing::info!("coordinator: {}", coordinator.info);
    tracing::info!("bob: {}", bob.info);

    let _alice_bg = alice.start().await.unwrap();
    let _coordinator_bg = coordinator.start().await.unwrap();
    let _bob_bg = bob.start().await.unwrap();

    // 2. Connect the nodes.

    tokio::time::sleep(Duration::from_secs(2)).await;
    alice.keep_connected(coordinator.info).await.unwrap();
    bob.keep_connected(coordinator.info).await.unwrap();

    // 3. Fund the Bitcoin wallets of the nodes who will open a channel.
    {
        bob.fund(bitcoin::Amount::from_sat(1_000_000))
            .await
            .unwrap();
        coordinator
            .fund(bitcoin::Amount::from_sat(1_000_000))
            .await
            .unwrap();

        // we need to wait here for the wallet to sync properly
        tokio::time::sleep(Duration::from_secs(5)).await;

        coordinator.sync();
        let balance = coordinator.wallet.inner().get_balance().unwrap();
        tracing::info!(%balance, "coordinator's wallet balance");

        bob.sync();
        let balance = bob.wallet.inner().get_balance().unwrap();
        tracing::info!(%balance, "bob's wallet balance after calling faucet");
    }

    tracing::info!("Opening channel");

    // 4. Create channel between bob and coordinator.
    bob.open_channel(coordinator.info, 30000, 0).unwrap();

    tokio::time::sleep(Duration::from_secs(2)).await;

    // Add 1 confirmation required for the channel to get usable.
    // we are mining to a random address to no pollute the users wallets
    let address = Address::from_str("bcrt1qylgu6ffkp3p0m8tw8kp4tt2dmdh755f4r5dq7s")
        .expect("To be a valid address");
    fund_and_mine(address.clone(), bitcoin::Amount::from_sat(1000)).await;

    // Add 5 confirmations for the channel to get announced.
    for _ in 1..6 {
        fund_and_mine(address.clone(), bitcoin::Amount::from_sat(1000)).await;
    }

    tokio::time::sleep(Duration::from_secs(2)).await;

    let mut i = 0;

    let retries = 5;
    while i < retries {
        if i == 4 {
            panic!("No channel found after {retries} retries");
        }

        alice.sync();
        coordinator.sync();
        bob.sync();

        tracing::debug!("Checking if channel is open yet");

        if has_channel(&coordinator, &bob) {
            break;
        }

        tokio::time::sleep(Duration::from_secs(5)).await;
        i += 1;
    }

    tracing::info!("Channel open");

    log_channel_id(&coordinator, 0, "coordinator-bob");
    log_channel_id(&bob, 0, "bob-coordinator");

    // ~~~~~~~ Setup done ~~~~~~~

    let intercepted = coordinator.create_intercept_scid(alice.info.pubkey);
    let coordinator_node_id = coordinator.info.pubkey;

    // 5. Generate an invoice from the payer to the payee.
    let invoice_amount = 500;
    let invoice_expiry = 0; // an expiry of 0 means the invoice never expires.
    let invoice = alice
        .create_interceptable_invoice(
            invoice_amount,
            intercepted,
            coordinator_node_id,
            invoice_expiry,
            "Interceptable Invoice".into(),
        )
        .unwrap();

    alice.sync();
    coordinator.sync();
    bob.sync();

    tracing::info!(?invoice);

    // 6. Pay the invoice.
    bob.send_payment(&invoice).unwrap();

    tokio::time::sleep(Duration::from_secs(5)).await;

    coordinator.sync();
    let balance = coordinator.get_ldk_balance().unwrap();
    tracing::info!(?balance, "coordinator's wallet balance");

    bob.sync();
    let balance = bob.get_ldk_balance().unwrap();
    tracing::info!(?balance, "bob's wallet balance");

    alice.sync();
    let balance = alice.get_ldk_balance().unwrap();
    tracing::info!(?balance, "Alice's wallet balance");

    assert_eq!(balance.available, invoice_amount)
}
