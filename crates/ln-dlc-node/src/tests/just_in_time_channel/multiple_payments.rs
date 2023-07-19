use crate::node::Node;
use crate::tests::init_tracing;
use crate::tests::just_in_time_channel::create::send_interceptable_payment;
use crate::tests::setup_coordinator_payer_channel;

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn just_in_time_channel_with_multiple_payments() {
    init_tracing();

    // Arrange

    let payer = Node::start_test_app("payer").unwrap();
    let coordinator = Node::start_test_coordinator("coordinator").unwrap();
    let payee = Node::start_test_app("payee").unwrap();

    payer.connect(coordinator.info).await.unwrap();
    payee.connect(coordinator.info).await.unwrap();

    let payer_to_payee_invoice_amount = 5_000;
    let expected_coordinator_payee_channel_value =
        setup_coordinator_payer_channel(payer_to_payee_invoice_amount, &coordinator, &payer).await;

    // this creates the just in time channel between the coordinator and payee
    send_interceptable_payment(
        &payer,
        &payee,
        &coordinator,
        payer_to_payee_invoice_amount,
        Some(expected_coordinator_payee_channel_value),
    )
    .await
    .unwrap();

    // after creating the just-in-time channel. The coordinator should have exactly 2 channels.
    assert_eq!(coordinator.channel_manager.list_channels().len(), 2);

    // 3 consecutive payments, we divide by 5 to account for fees
    // TODO: Dividing by 4 should work but leads to rounding error because of how the fees are
    // calculated in the test assertions
    let consecutive_payment_invoice_amount = payer_to_payee_invoice_amount / 5;

    for _ in 0..3 {
        send_interceptable_payment(
            &payer,
            &payee,
            &coordinator,
            consecutive_payment_invoice_amount,
            None,
        )
        .await
        .unwrap();

        // no additional just-in-time channel should be created.
        assert_eq!(coordinator.channel_manager.list_channels().len(), 2);
    }
}
