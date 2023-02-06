use crate::node::Node;
use crate::tests::fund_and_mine;
use crate::tests::Faucet;
use bitcoin::Network;
use dlc_manager::Wallet;
use rand::thread_rng;
use rand::RngCore;
use std::time::Duration;
use tracing_subscriber::util::SubscriberInitExt;

#[tokio::test]
async fn given_sibling_channel_when_payment_then_can_be_claimed() {
    let _guard = tracing_subscriber::fmt().with_test_writer().set_default();

    // 1. Set up two LN-DLC nodes.
    let alice = {
        let seed = [
            137, 78, 181, 39, 89, 143, 9, 224, 92, 125, 51, 183, 87, 95, 206, 236, 135, 33, 54, 10,
            237, 169, 132, 74, 230, 66, 244, 244, 89, 224, 23, 62,
        ];

        let mut ephemeral_randomness = [0; 32];
        thread_rng().fill_bytes(&mut ephemeral_randomness);

        // todo: the tests are executed in the crates/ln-dlc-node directory, hence the folder will
        // be created their. but the creation will fail if the .ldk-data/alice/on_chain has not been
        // created before.
        Node::new(
            Network::Regtest,
            ".ldk-data/alice".to_string(),
            format!("127.0.0.1:8005")
                .parse()
                .expect("Hard-coded IP and port to be valid"),
            "tcp://localhost:50000".to_string(),
            seed,
            ephemeral_randomness,
        )
        .await
    };
    tracing::info!("Alice: {}", alice.info);

    let bob = {
        let mut seed = [0; 32];
        thread_rng().fill_bytes(&mut seed);

        let mut ephemeral_randomness = [0; 32];
        thread_rng().fill_bytes(&mut ephemeral_randomness);

        Node::new(
            Network::Regtest,
            ".ldk-data/bob".to_string(),
            format!("127.0.0.1:8006")
                .parse()
                .expect("Hard-coded IP and port to be valid"),
            "tcp://localhost:50000".to_string(),
            seed,
            ephemeral_randomness,
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
    let address = alice.wallet.get_new_address().unwrap().to_string();
    fund_and_mine(&Faucet {
        address,
        amount: 0.1,
    })
    .await;
    alice.wallet.inner().sync(vec![]).unwrap();
    tracing::info!("{}", alice.wallet.inner().get_balance().unwrap());

    tracing::info!("Opening channel");

    // 4. Create channel between them.
    alice.open_channel(bob.info, 30000, 0).unwrap();

    // 5. Generate an invoice from the payer to the payee.
    // 6. Pay the invoice.
    // 7. Claim the payment.
}
