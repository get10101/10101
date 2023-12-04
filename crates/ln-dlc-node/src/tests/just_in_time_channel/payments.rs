use crate::node::Node;
use crate::tests::calculate_routing_fee_msat;
use crate::tests::init_tracing;
use crate::tests::just_in_time_channel::create::send_interceptable_payment;
use crate::tests::just_in_time_channel::create::send_payment;
use crate::tests::setup_coordinator_payer_channel;
use crate::tests::wait_for_n_usable_channels;
use crate::tests::wait_until;
use std::time::Duration;

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

    let payer_to_payee_invoice_amount = 25_000;
    let (expected_coordinator_payee_channel_value, liquidity_request) =
        setup_coordinator_payer_channel(
            &coordinator,
            &payer,
            payee.info.pubkey,
            payer_to_payee_invoice_amount,
        )
        .await;

    // this creates the just in time channel between the coordinator and payee
    send_interceptable_payment(
        &payer,
        &payee,
        &coordinator,
        payer_to_payee_invoice_amount,
        liquidity_request,
        expected_coordinator_payee_channel_value,
    )
    .await
    .unwrap();

    // after creating the just-in-time channel. The coordinator should have exactly 2 usable
    // channels with short channel ids.
    wait_for_n_usable_channels(2, &coordinator).await.unwrap();

    // 3 consecutive payments, we divide by 5 to account for fees
    // Note: Dividing by 4 should work but leads to rounding error because of how the fees are
    // calculated in the test assertions
    let consecutive_payment_invoice_amount = payer_to_payee_invoice_amount / 5;

    for _ in 0..3 {
        send_payment(
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

    let opening_invoice_amount = 60_000;
    let (expected_coordinator_payee_channel_value, liquidity_request) =
        setup_coordinator_payer_channel(
            &coordinator,
            &payer,
            payee.info.pubkey,
            opening_invoice_amount,
        )
        .await;

    send_interceptable_payment(
        &payer,
        &payee,
        &coordinator,
        opening_invoice_amount,
        liquidity_request,
        expected_coordinator_payee_channel_value,
    )
    .await
    .unwrap();

    // after creating the just-in-time channel. The coordinator should have exactly 2 usable
    // channels with short channel ids.
    wait_for_n_usable_channels(2, &coordinator).await.unwrap();

    // Act

    let coordinator_balance_before = coordinator.get_ldk_balance().available_msat();

    let mut ldk_config_coordinator = *coordinator.ldk_config.read();

    ldk_config_coordinator
        .channel_config
        .forwarding_fee_proportional_millionths *= 10;

    let fee_rate = ldk_config_coordinator
        .channel_config
        .forwarding_fee_proportional_millionths;

    coordinator.update_ldk_settings(ldk_config_coordinator);

    wait_until(Duration::from_secs(30), || async {
        let payer_channels = payer.channel_manager.list_channels();
        Ok(payer_channels
            .iter()
            .any(|channel| {
                channel
                    .counterparty
                    .forwarding_info
                    .clone()
                    .expect("to have forwarding info")
                    .fee_proportional_millionths
                    == fee_rate
            })
            .then_some(()))
    })
    .await
    .expect("all channels to have updated fee");

    let payment_amount_sat = 5_000;
    send_payment(&payer, &payee, &coordinator, payment_amount_sat, None)
        .await
        .unwrap();

    // Assert

    let coordinator_balance_after = coordinator.get_ldk_balance().available_msat();
    let routing_fee_charged_msat = coordinator_balance_after - coordinator_balance_before;

    let routing_fee_expected_msat =
        calculate_routing_fee_msat(ldk_config_coordinator.channel_config, payment_amount_sat);

    assert_eq!(routing_fee_charged_msat, routing_fee_expected_msat);
}
