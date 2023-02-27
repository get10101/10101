use crate::node::Node;
use crate::tests::init_tracing;
use bitcoin::Amount;
use std::time::Duration;

#[tokio::test]
#[ignore]
async fn single_hop_payment() {
    init_tracing();

    // Arrange

    let payer = Node::start_test_app("payer").await.unwrap();
    let payee = Node::start_test_app("payee").await.unwrap();

    payer.keep_connected(payee.info).await.unwrap();

    payer.fund(Amount::from_btc(0.1).unwrap()).await.unwrap();

    payer.open_channel(&payee.info, 30_000, 0).await.unwrap();

    let payer_balance_before = payer.get_ldk_balance();
    let payee_balance_before = payee.get_ldk_balance();

    // No mining step needed because the channels are _implicitly_
    // configured to support 0-conf

    // Act

    let invoice_amount = 3_000;
    let invoice = payee.create_invoice(invoice_amount).unwrap();

    payer.send_payment(&invoice).unwrap();

    // For the payment to be claimed before the wallet syncs
    tokio::time::sleep(Duration::from_secs(1)).await;

    payer.sync();
    payee.sync();

    // Assert

    let payer_balance_after = payer.get_ldk_balance();
    let payee_balance_after = payee.get_ldk_balance();

    assert_eq!(
        payer_balance_before.available - payer_balance_after.available,
        invoice_amount
    );

    assert_eq!(
        payee_balance_after.available - payee_balance_before.available,
        invoice_amount
    );
}
