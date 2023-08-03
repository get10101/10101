use crate::node::Node;
use crate::tests::calculate_routing_fee_msat;
use crate::tests::init_tracing;
use crate::tests::just_in_time_channel::create::send_interceptable_payment;
use crate::tests::setup_coordinator_payer_channel;

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn just_in_time_channel_with_multiple_payments() {
    init_tracing();

    // Arrange

    let (payer, _running_payer) = Node::start_test_app("payer").unwrap();
    let (coordinator, _running_coord) = Node::start_test_coordinator("coordinator").unwrap();
    let (payee, _running_payee) = Node::start_test_app("payee").unwrap();

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

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn new_config_affects_routing_fees() {
    init_tracing();

    // Arrange

    let (payer, _running_payer) = Node::start_test_app("payer").unwrap();
    let (coordinator, _running_coord) = Node::start_test_coordinator("coordinator").unwrap();
    let (payee, _running_payee) = Node::start_test_app("payee").unwrap();

    payer.connect(coordinator.info).await.unwrap();
    payee.connect(coordinator.info).await.unwrap();

    let opening_invoice_amount = 10_000;
    let expected_coordinator_payee_channel_value =
        setup_coordinator_payer_channel(opening_invoice_amount, &coordinator, &payer).await;

    send_interceptable_payment(
        &payer,
        &payee,
        &coordinator,
        opening_invoice_amount,
        Some(expected_coordinator_payee_channel_value),
    )
    .await
    .unwrap();

    // Act

    let coordinator_balance_before = coordinator.get_ldk_balance().available_msat();

    let mut ldk_config_coordinator = *coordinator.ldk_config.read();

    ldk_config_coordinator
        .channel_config
        .forwarding_fee_proportional_millionths *= 10;

    coordinator.update_ldk_settings(ldk_config_coordinator);

    let payment_amount_sat = 5_000;
    send_interceptable_payment(&payer, &payee, &coordinator, payment_amount_sat, None)
        .await
        .unwrap();

    // Assert

    let coordinator_balance_after = coordinator.get_ldk_balance().available_msat();
    let routing_fee_charged_msat = coordinator_balance_after - coordinator_balance_before;

    let routing_fee_expected_msat =
        calculate_routing_fee_msat(ldk_config_coordinator.channel_config, payment_amount_sat);

    assert_eq!(routing_fee_charged_msat, routing_fee_expected_msat);
}
