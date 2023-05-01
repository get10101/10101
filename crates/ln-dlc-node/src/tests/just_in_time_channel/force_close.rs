use crate::ln::JUST_IN_TIME_CHANNEL_OUTBOUND_LIQUIDITY_SAT;
use crate::node::Node;
use crate::tests::bitcoind;
use crate::tests::init_tracing;
use crate::tests::just_in_time_channel::create::send_interceptable_payment;
use crate::tests::just_in_time_channel::TestPath;
use crate::tests::min_outbound_liquidity_channel_creator;
use bitcoin::Amount;
use dlc_manager::subchannel::LNChannelManager;

#[tokio::test]
#[ignore]
async fn force_close() {
    init_tracing();

    // Arrange

    let payer = Node::start_test_app("payer").await.unwrap();
    let coordinator = Node::start_test_coordinator("coordinator").await.unwrap();
    let payee = Node::start_test_app("payee").await.unwrap();

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
        TestPath::MobileFunding,
        &payer,
        &payee,
        &coordinator,
        invoice_amount,
        Some(JUST_IN_TIME_CHANNEL_OUTBOUND_LIQUIDITY_SAT),
    )
    .await
    .unwrap();

    let channel_id = payee
        .channel_manager
        .list_usable_channels()
        .first()
        .unwrap()
        .channel_id;

    assert_eq!(payee.get_on_chain_balance().unwrap().confirmed, 0);
    assert_eq!(payee.get_ldk_balance().available, 1_000);
    assert_eq!(payee.get_ldk_balance().pending_close, 0);

    payee
        .channel_manager
        .force_close_channel(&channel_id, &coordinator.info.pubkey)
        .unwrap();

    payee.sync().unwrap();

    assert_eq!(payee.get_on_chain_balance().unwrap().confirmed, 0);
    assert_eq!(payee.get_ldk_balance().available, 0);
    assert_eq!(payee.get_ldk_balance().pending_close, 1_000);

    // the delay we have to wait before the fund can be claimed on chain again.
    bitcoind::mine(144).await.unwrap();

    // this sync triggers the `[Event::SpendableOutputs]` broadcasting the transaction to claim the
    // payees coins.
    payee.sync().unwrap();

    // mine a single block to claim the spendable output after waiting for the force close delay.
    bitcoind::mine(1).await.unwrap();
    payee.sync().unwrap();

    // 1_000 - 122 fees = 878 sats
    assert_eq!(payee.get_on_chain_balance().unwrap().confirmed, 878);
    assert_eq!(payee.get_ldk_balance().available, 0);
    assert_eq!(payee.get_ldk_balance().pending_close, 0);
}
