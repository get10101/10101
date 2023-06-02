use crate::ln::JUST_IN_TIME_CHANNEL_OUTBOUND_LIQUIDITY_SAT;
use crate::node::Node;
use crate::tests::init_tracing;
use crate::tests::just_in_time_channel::create::send_interceptable_payment;
use crate::tests::just_in_time_channel::TestPath;
use crate::tests::min_outbound_liquidity_channel_creator;
use bitcoin::Amount;

#[tokio::test]
#[ignore]
async fn offline_receiver() {
    init_tracing();

    // Arrange

    let payer = Node::start_test_app("payer").unwrap();
    let coordinator = Node::start_test_coordinator("coordinator").unwrap();
    let payee = Node::start_test_app("payee").unwrap();

    payer.connect(coordinator.info).await.unwrap();

    coordinator.fund(Amount::from_sat(1_000_000)).await.unwrap();

    let payer_outbound_liquidity_sat = 25_000;
    let coordinator_outbound_liquidity_sat =
        min_outbound_liquidity_channel_creator(&payer, payer_outbound_liquidity_sat);

    coordinator
        .open_channel(
            &payer,
            coordinator_outbound_liquidity_sat,
            payer_outbound_liquidity_sat,
        )
        .await
        .unwrap();

    let invoice_amount = 1_000;

    send_interceptable_payment(
        TestPath::FundingThroughMobile,
        &payer,
        &payee,
        &coordinator,
        invoice_amount,
        Some(JUST_IN_TIME_CHANNEL_OUTBOUND_LIQUIDITY_SAT),
    )
    .await
    .unwrap();
}
