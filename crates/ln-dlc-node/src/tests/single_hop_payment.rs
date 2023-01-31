//! Do this one first.
//! TODO: Might be called no-hop (hop[e]less).

use super::start_ln_dlc_node;

#[tokio::test]
async fn given_sibling_channel_when_payment_then_can_be_claimed() {
    // 1. Set up two LN-DLC nodes.
    let _alice = start_ln_dlc_node().await;
    let _bob = start_ln_dlc_node().await;

    // 2. Connect the two nodes.
    todo!("Implement connection between both nodes");

    // 3. Fund the Bitcoin wallet of one of the nodes (the payer).
    // 4. Create channel between them.
    // 5. Generate an invoice from the payer to the payee.
    // 6. Pay the invoice.
    // 7. Claim the payment.
}
