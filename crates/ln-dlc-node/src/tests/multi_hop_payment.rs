use crate::node::Node;
use crate::seed::Bip39Seed;
use crate::tests::create_tmp_dir;
use crate::tests::fund_and_mine;
use crate::tests::init_tracing;
use crate::tests::ELECTRS_ORIGIN;
use bip39::Mnemonic;
use bitcoin::Network;
use dlc_manager::Wallet;
use rand::thread_rng;
use rand::RngCore;
use std::time::Duration;

#[tokio::test]
async fn multi_hop_payment() {
    init_tracing();

    let test_dir = create_tmp_dir("multi_hop_test");

    // 1. Set up two LN-DLC nodes.
    let alice = {
        let data_dir = test_dir.join("alice");

        let seed = Bip39Seed::from(
            Mnemonic::parse(
                "tray lift outside jump romance whale bag snake gadget disease chunk erupt",
            )
            .expect("To be a valid mnemonic"),
        );

        let mut ephemeral_randomness = [0; 32];
        thread_rng().fill_bytes(&mut ephemeral_randomness);

        // todo: the tests are executed in the crates/ln-dlc-node directory, hence the folder will
        // be created there. but the creation will fail if the .ldk-data/alice/on_chain has not been
        // created before.
        Node::new(
            "Alice".to_string(),
            Network::Regtest,
            data_dir.as_path(),
            "127.0.0.1:8010"
                .parse()
                .expect("Hard-coded IP and port to be valid"),
            ELECTRS_ORIGIN.to_string(),
            seed,
            ephemeral_randomness,
        )
        .await
    };

    let bob = {
        let data_dir = test_dir.join("bob");

        let seed = Bip39Seed::from(
            Mnemonic::parse(
                "wish wealth video hello nose local ordinary nasty aisle behave casino fog",
            )
            .expect("To be a valid mnemonic"),
        );

        let mut ephemeral_randomness = [0; 32];
        thread_rng().fill_bytes(&mut ephemeral_randomness);

        Node::new(
            "Bob".to_string(),
            Network::Regtest,
            data_dir.as_path(),
            "127.0.0.1:8011"
                .parse()
                .expect("Hard-coded IP and port to be valid"),
            ELECTRS_ORIGIN.to_string(),
            seed,
            ephemeral_randomness,
        )
        .await
    };

    let claire = {
        let data_dir = test_dir.join("claire");

        let seed = Bip39Seed::from(
            Mnemonic::parse(
                "stay mistake gas defy bleak whisper empower elephant gate priority craft earth",
            )
            .expect("To be a valid mnemonic"),
        );

        let mut ephemeral_randomness = [0; 32];
        thread_rng().fill_bytes(&mut ephemeral_randomness);

        Node::new(
            "Claire".to_string(),
            Network::Regtest,
            data_dir.as_path(),
            "127.0.0.1:8012"
                .parse()
                .expect("Hard-coded IP and port to be valid"),
            ELECTRS_ORIGIN.to_string(),
            seed,
            ephemeral_randomness,
        )
        .await
    };
    tracing::info!("Alice: {}", alice.info);
    tracing::info!("Bob: {}", bob.info);
    tracing::info!("Claire: {}", claire.info);

    let _alice_bg = alice.start().await.unwrap();
    let _bob_bg = bob.start().await.unwrap();
    let _claire_bg = claire.start().await.unwrap();

    // 2. Connect the two nodes.

    // TODO: Remove sleep by allowing the first connection attempt to be retried
    tokio::time::sleep(Duration::from_secs(2)).await;
    alice.keep_connected(bob.info).await.unwrap();
    claire.keep_connected(bob.info).await.unwrap();
    alice.keep_connected(claire.info).await.unwrap();

    // 3. Fund the Bitcoin wallets of the nodes who will open a channel.
    {
        alice
            .fund(bitcoin::Amount::from_sat(1_000_000))
            .await
            .unwrap();
        bob.fund(bitcoin::Amount::from_sat(1_000_000))
            .await
            .unwrap();

        // we need to wait here for the wallet to sync properly
        tokio::time::sleep(Duration::from_secs(5)).await;

        alice.sync();
        let balance = alice.wallet.inner().get_balance().unwrap();
        tracing::info!(%balance, "Alice's wallet balance after calling the faucet");

        bob.sync();
        let balance = bob.wallet.inner().get_balance().unwrap();
        tracing::info!(%balance, "Bob's wallet balance after calling the faucet");

        claire.sync();
        let balance = claire.wallet.inner().get_balance().unwrap();
        tracing::info!(%balance, "Claire's wallet balance after calling the faucet");
    }

    tracing::info!("Opening channel");

    // 4. Create channel between alice and bob.
    alice.open_channel(bob.info, 30000, 0).unwrap();
    // 4. Create channel between bob and claire.
    bob.open_channel(claire.info, 30000, 0).unwrap();

    tokio::time::sleep(Duration::from_secs(2)).await;

    // Add 1 confirmation required for the channel to get usable.
    let address = alice.wallet.get_new_address().unwrap();
    fund_and_mine(address, bitcoin::Amount::from_sat(1000)).await;

    // Add 5 confirmations for the channel to get announced.
    for _ in 1..6 {
        let address = alice.wallet.get_new_address().unwrap();
        fund_and_mine(address, bitcoin::Amount::from_sat(1000)).await;
    }

    tokio::time::sleep(Duration::from_secs(2)).await;

    // TODO: it would be nicer if we could hook that assertion to the corresponding event received
    // through the event handler.
    loop {
        alice.sync();
        bob.sync();
        claire.sync();

        tracing::debug!("Checking if channel is open yet");

        if has_channel(&alice, &bob) && has_channel(&bob, &claire) {
            break;
        }

        tokio::time::sleep(Duration::from_secs(5)).await;
    }

    tracing::info!("Channel open");

    log_channel_id(&alice, 0, "alice-bob");
    log_channel_id(&bob, 0, "bob-alice");
    log_channel_id(&bob, 1, "bob-claire");
    log_channel_id(&claire, 0, "claire-bob");

    // 5. Generate an invoice from the payer to the payee.
    let invoice_amount = 500;
    let invoice = claire.create_invoice(invoice_amount).unwrap();

    alice.sync();
    bob.sync();
    claire.sync();

    tracing::info!(?invoice);

    // 6. Pay the invoice.
    alice.send_payment(&invoice).unwrap();

    tokio::time::sleep(Duration::from_secs(5)).await;

    alice.sync();
    let balance = alice.get_ldk_balance().unwrap();
    tracing::info!(?balance, "Alice's wallet balance");

    bob.sync();
    let balance = bob.get_ldk_balance().unwrap();
    tracing::info!(?balance, "Bob's wallet balance");

    claire.sync();
    let balance = claire.get_ldk_balance().unwrap();
    tracing::info!(?balance, "Claire's wallet balance");

    assert_eq!(balance.available, invoice_amount)
}

fn has_channel(source_node: &Node, target_node: &Node) -> bool {
    source_node
        .channel_manager()
        .list_channels()
        .iter()
        .any(|channel| {
            channel.counterparty.node_id == target_node.channel_manager().get_our_node_id()
                && channel.is_usable
        })
}

fn log_channel_id(node: &Node, index: usize, pair: &str) {
    let details = node
        .channel_manager()
        .list_channels()
        .get(index)
        .unwrap()
        .clone();

    let channel_id = hex::encode(details.channel_id);
    let short_channel_id = details.short_channel_id.unwrap();
    let is_ready = details.is_channel_ready;
    let is_usable = details.is_usable;
    tracing::info!(channel_id, short_channel_id, is_ready, is_usable, "{pair}");
}
