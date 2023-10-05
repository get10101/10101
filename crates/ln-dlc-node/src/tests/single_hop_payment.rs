use crate::node::Node;
use crate::tests::init_tracing;
use crate::tests::wait_for_n_usable_channels;
use bitcoin::Amount;

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn single_hop_payment() {
    init_tracing();

    // Arrange

    let (payer, _running_payer) = Node::start_test_app("payer").unwrap();
    let (payee, _running_payee) = Node::start_test_app("payee").unwrap();

    payer.connect(payee.info).await.unwrap();

    payer.fund(Amount::from_btc(0.1).unwrap()).await.unwrap();

    payer.open_private_channel(&payee, 30_000, 0).await.unwrap();

    // after creating the just-in-time channel. The coordinator should have exactly 2 usable
    // channels with short channel ids.
    wait_for_n_usable_channels(1, &payer).await.unwrap();

    let payer_balance_before = payer.get_ldk_balance();
    let payee_balance_before = payee.get_ldk_balance();

    // No mining step needed because the channels are _implicitly_
    // configured to support 0-conf

    // Act

    let invoice_amount = 3_000;
    let invoice = payee
        .create_invoice(invoice_amount, "".to_string(), 180)
        .unwrap();

    payer.send_payment(&invoice).unwrap();

    payee
        .wait_for_payment_claimed(invoice.payment_hash())
        .await
        .unwrap();

    // Assert

    // Sync LN wallet after payment is claimed to update the balances
    payer.sync_on_chain().await.unwrap();
    payee.sync_on_chain().await.unwrap();

    let payer_balance_after = payer.get_ldk_balance();
    let payee_balance_after = payee.get_ldk_balance();

    assert_eq!(
        payer_balance_before.available() - payer_balance_after.available(),
        invoice_amount
    );

    assert_eq!(
        payee_balance_after.available() - payee_balance_before.available(),
        invoice_amount
    );
}
