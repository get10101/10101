//! Do this one first.
//! TODO: Might be called no-hop (hop[e]less).

use bitcoin::Network;
use rand::thread_rng;
use rand::RngCore;

use crate::node::Node;

#[tokio::test]
async fn given_sibling_channel_when_payment_then_can_be_claimed() {
    // 1. Set up two LN-DLC nodes.
    let alice = {
        let mut seed = [0; 32];
        thread_rng().fill_bytes(&mut seed);

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
            "http://localhost:30000/".to_string(),
            seed,
            ephemeral_randomness,
        )
        .await
    };
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
            "http://localhost:30000/".to_string(),
            seed,
            ephemeral_randomness,
        )
        .await
    };

    alice.start().await.unwrap();
    bob.start().await.unwrap();

    // 2. Connect the two nodes.

    alice.connect(bob).await.unwrap();

    // 3. Fund the Bitcoin wallet of one of the nodes (the payer).
    // 4. Create channel between them.
    // 5. Generate an invoice from the payer to the payee.
    // 6. Pay the invoice.
    // 7. Claim the payment.
}
