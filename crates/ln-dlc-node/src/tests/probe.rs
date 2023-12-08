use crate::node::Node;
use crate::tests::bitcoind::mine;
use crate::tests::init_tracing;
use bitcoin::Amount;
use lightning::ln::channelmanager::ChannelDetails;
use std::time::Duration;

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn estimate_payment_fee() {
    init_tracing();

    // Arrange

    let (payer, _running_payer) = Node::start_test_app("payer").unwrap();
    let (a, _running_coord) = Node::start_test_coordinator("a").unwrap();
    let (b, _running_coord) = Node::start_test_coordinator("b").unwrap();
    let (c, _running_coord) = Node::start_test_coordinator("c").unwrap();
    let (d, _running_coord) = Node::start_test_coordinator("d").unwrap();
    let (payee, _running_payee) = Node::start_test_app("payee").unwrap();

    payer.connect(a.info).await.unwrap();
    a.connect(b.info).await.unwrap();
    b.connect(c.info).await.unwrap();
    c.connect(d.info).await.unwrap();
    d.connect(payee.info).await.unwrap();

    payer.fund(Amount::from_sat(100_000)).await.unwrap();
    a.fund(Amount::from_sat(100_000)).await.unwrap();
    b.fund(Amount::from_sat(100_000)).await.unwrap();
    c.fund(Amount::from_sat(100_000)).await.unwrap();
    d.fund(Amount::from_sat(100_000)).await.unwrap();

    payer.open_public_channel(&a, 20_000, 20_000).await.unwrap();
    let channel1 = a.open_public_channel(&b, 20_000, 20_000).await.unwrap();
    let channel2 = b.open_public_channel(&c, 20_000, 20_000).await.unwrap();
    let channel3 = c.open_public_channel(&d, 20_000, 20_000).await.unwrap();
    d.open_private_channel(&payee, 20_000, 20_000)
        .await
        .unwrap();

    mine(6).await.unwrap();

    tracing::info!("Waiting for channels to be discovered");

    for _ in 0..5 {
        payer.sync_on_chain().await.unwrap();
        a.sync_on_chain().await.unwrap();
        b.sync_on_chain().await.unwrap();
        c.sync_on_chain().await.unwrap();
        d.sync_on_chain().await.unwrap();
        payee.sync_on_chain().await.unwrap();

        payer.broadcast_node_announcement();
        a.broadcast_node_announcement();
        b.broadcast_node_announcement();
        c.broadcast_node_announcement();
        d.broadcast_node_announcement();
        payee.broadcast_node_announcement();

        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }

    tracing::info!("Done waiting for channels to be discovered");

    // Successful fee estimate

    // Act

    let invoice_amount_sat = 5_000;
    let invoice = payee
        .create_invoice(invoice_amount_sat, "".to_string(), 180)
        .unwrap();

    let estimated_fee_msat = payer
        .estimate_payment_fee_msat(invoice, None, Duration::from_secs(60))
        .await
        .unwrap();

    // Assert

    // We only consider the 3 intermediate hops because the `payee`-to-`a` channel does not take a
    // fee and the last one is a private channel which is unannounced, meaning the probe will not
    // try going all the way there.
    let expected_fee_msat = calculate_fee_msat(channel1, invoice_amount_sat)
        + calculate_fee_msat(channel2, invoice_amount_sat)
        + calculate_fee_msat(channel3, invoice_amount_sat);

    assert_eq!(estimated_fee_msat, expected_fee_msat);

    // Failed fee estimate due to route-not-found caused by insufficient liquidity

    // Act

    let invoice_amount_sat = 50_000;
    let invoice = payee
        .create_invoice(invoice_amount_sat, "".to_string(), 180)
        .unwrap();

    let res = payer
        .estimate_payment_fee_msat(invoice, None, Duration::from_secs(60))
        .await;

    // Assert

    assert!(res.is_err())
}

fn calculate_fee_msat(channel_details: ChannelDetails, invoice_amount_sat: u64) -> u64 {
    let invoice_amount_msat = (invoice_amount_sat * 1_000) as f32;

    let forwarding_fee_proportional_millionths = channel_details
        .config
        .unwrap()
        .forwarding_fee_proportional_millionths
        as f32;
    let fee_as_percentage_of_invoice_amount_msat =
        forwarding_fee_proportional_millionths / 1_000_000.0;

    let proportional_fee_msat =
        (fee_as_percentage_of_invoice_amount_msat * invoice_amount_msat) as u64;

    let flat_fee_msat = channel_details.config.unwrap().forwarding_fee_base_msat as u64;

    proportional_fee_msat + flat_fee_msat
}
