use crate::node::Node;
use crate::seed::Bip39Seed;
use crate::tests::system_tests::create_tmp_dir;
use crate::tests::system_tests::fund_and_mine;
use crate::tests::system_tests::init_tracing;
use crate::tests::system_tests::ELECTRS_ORIGIN;
use bitcoin::Network;
use dlc_manager::Wallet;
use lightning::util::events::Event;
use rand::thread_rng;
use rand::RngCore;
use std::sync::mpsc::channel;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::time::Duration;

#[tokio::test]
async fn given_sibling_channel_when_payment_then_can_be_claimed() {
    init_tracing();

    let test_dir = create_tmp_dir("single_hop_test");

    let (alice_events_sender, alice_events_receiver): (Sender<Event>, Receiver<Event>) = channel();
    let (bob_events_sender, _bob_events_receiver): (Sender<Event>, Receiver<Event>) = channel();

    // 1. Set up two LN-DLC nodes.
    let alice = {
        let data_dir = test_dir.join("alice");

        let seed = Bip39Seed::new().expect("A valid bip39 seed");

        let mut ephemeral_randomness = [0; 32];
        thread_rng().fill_bytes(&mut ephemeral_randomness);

        // todo: the tests are executed in the crates/ln-dlc-node directory, hence the folder will
        // be created there. but the creation will fail if the .ldk-data/alice/on_chain has not been
        // created before.
        Node::new(
            "Alice".to_string(),
            Network::Regtest,
            data_dir.as_path(),
            "127.0.0.1:8005"
                .parse()
                .expect("Hard-coded IP and port to be valid"),
            ELECTRS_ORIGIN.to_string(),
            seed,
            ephemeral_randomness,
            alice_events_sender,
        )
        .await
    };
    tracing::info!("Alice: {}", alice.info);

    let bob = {
        let data_dir = test_dir.join("bob");
        let seed = Bip39Seed::new().expect("A valid bip39 seed");

        let mut ephemeral_randomness = [0; 32];
        thread_rng().fill_bytes(&mut ephemeral_randomness);

        Node::new(
            "Bob".to_string(),
            Network::Regtest,
            data_dir.as_path(),
            "127.0.0.1:8006"
                .parse()
                .expect("Hard-coded IP and port to be valid"),
            ELECTRS_ORIGIN.to_string(),
            seed,
            ephemeral_randomness,
            bob_events_sender,
        )
        .await
    };
    tracing::info!("Bob: {}", bob.info);

    let _alice_bg = alice.start().await.unwrap();
    let _bob_bg = bob.start().await.unwrap();

    // 2. Connect the two nodes.

    // TODO: Remove sleep by allowing the first connection attempt to be retried
    tokio::time::sleep(Duration::from_secs(2)).await;
    alice.keep_connected(bob.info).await.unwrap();

    // 3. Fund the Bitcoin wallet of one of the nodes (the payer).
    alice
        .fund(bitcoin::Amount::from_btc(0.1).unwrap())
        .await
        .unwrap();

    tracing::info!("Opening channel");

    // 4. Create channel between them.
    alice.open_channel(bob.info, 30000, 0).unwrap();

    tokio::time::sleep(Duration::from_secs(2)).await;

    // Add 1 confirmations required for the channel to get usable.
    let address = alice.wallet.get_new_address().unwrap();
    fund_and_mine(address.clone(), bitcoin::Amount::from_sat(1000)).await;

    let mut iter = alice_events_receiver.try_iter();
    loop {
        match iter.next() {
            Some(event) => {
                if let Event::ChannelReady { .. } = event {
                    println!("Channel ready mate");
                    break;
                }
            }
            None => {
                println!("Waiting for next event");
                alice.sync();
                bob.sync();
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }

    tracing::info!("Channel open");

    // 5. Generate an invoice from the payer to the payee.
    let invoice_amount = 3000;
    let invoice = bob.create_invoice(invoice_amount).unwrap();
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

    assert_eq!(balance.available, invoice_amount)
}
