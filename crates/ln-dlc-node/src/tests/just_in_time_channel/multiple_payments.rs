use crate::ln::JUST_IN_TIME_CHANNEL_OUTBOUND_LIQUIDITY_SAT;
use crate::node::Node;
use crate::tests::init_tracing;
use crate::tests::just_in_time_channel::create::send_interceptable_payment;
use crate::tests::min_outbound_liquidity_channel_creator;
use bitcoin::Amount;

#[tokio::test]
#[ignore]
async fn just_in_time_channel_with_multiple_payments() {
    init_tracing();

    // Arrange

    let user_a = Node::start_test_app("user_a").await.unwrap();
    let coordinator = Node::start_test_coordinator("coordinator").await.unwrap();
    let user_b = Node::start_test_app("user_b").await.unwrap();

    user_a.connect(coordinator.info).await.unwrap();
    user_b.connect(coordinator.info).await.unwrap();

    coordinator.fund(Amount::from_sat(100_000)).await.unwrap();

    let payer_outbound_liquidity_sat = 25_000;
    let coordinator_outbound_liquidity_sat =
        min_outbound_liquidity_channel_creator(&user_a, payer_outbound_liquidity_sat);

    coordinator
        .open_channel(
            &user_a,
            coordinator_outbound_liquidity_sat,
            payer_outbound_liquidity_sat,
        )
        .await
        .unwrap();

    // this creates the just in time channel between the coordinator and user_b
    send_interceptable_payment(
        &user_a,
        &user_b,
        &coordinator,
        5_000,
        Some(JUST_IN_TIME_CHANNEL_OUTBOUND_LIQUIDITY_SAT),
    )
    .await
    .unwrap();

    // after creating the just-in-time channel. The coordinator should have exactly 2 channels.
    assert_eq!(coordinator.channel_manager.list_channels().len(), 2);

    send_interceptable_payment(&user_a, &user_b, &coordinator, 3_000, None)
        .await
        .unwrap();

    // no additional just-in-time channel should be created.
    assert_eq!(coordinator.channel_manager.list_channels().len(), 2);

    send_interceptable_payment(&user_b, &user_a, &coordinator, 4_500, None)
        .await
        .unwrap();

    // no additional just-in-time channel should be created.
    assert_eq!(coordinator.channel_manager.list_channels().len(), 2);

    send_interceptable_payment(&user_a, &user_b, &coordinator, 5_000, None)
        .await
        .unwrap();

    // no additional just-in-time channel should be created.
    assert_eq!(coordinator.channel_manager.list_channels().len(), 2);
}
