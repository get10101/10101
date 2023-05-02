use crate::ln::JUST_IN_TIME_CHANNEL_OUTBOUND_LIQUIDITY_SAT;
use crate::node::Node;
use crate::node::PaymentMap;
use crate::tests::bitcoind;
use crate::tests::init_tracing;
use crate::tests::just_in_time_channel::create::send_interceptable_payment;
use crate::tests::just_in_time_channel::TestPathChannelClose;
use crate::tests::just_in_time_channel::TestPathFunding;
use crate::tests::min_outbound_liquidity_channel_creator;
use anyhow::anyhow;
use anyhow::Result;
use bitcoin::Amount;
use dlc_manager::subchannel::LNChannelManager;
use std::time::Duration;

#[tokio::test]
#[ignore]
async fn collab_close() {
    init_tracing();

    // Arrange

    let payer = Node::start_test_app("payer").await.unwrap();
    let coordinator = Node::start_test_coordinator("coordinator").await.unwrap();
    let payee = Node::start_test_app("payee").await.unwrap();

    payer.connect(coordinator.info).await.unwrap();
    payee.connect(coordinator.info).await.unwrap();

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
        TestPathFunding::Online,
        &payer,
        &payee,
        &coordinator,
        invoice_amount,
        Some(JUST_IN_TIME_CHANNEL_OUTBOUND_LIQUIDITY_SAT),
    )
    .await
    .unwrap();

    close_channel(
        TestPathChannelClose::Collaborative,
        &payee,
        &coordinator,
        invoice_amount,
    )
    .await
    .unwrap();
}

#[tokio::test]
#[ignore]
async fn force_close() {
    init_tracing();

    // Arrange

    let payer = Node::start_test_app("payer").await.unwrap();
    let coordinator = Node::start_test_coordinator("coordinator").await.unwrap();
    let payee = Node::start_test_app("payee").await.unwrap();

    payer.connect(coordinator.info).await.unwrap();
    payee.connect(coordinator.info).await.unwrap();

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
        TestPathFunding::Online,
        &payer,
        &payee,
        &coordinator,
        invoice_amount,
        Some(JUST_IN_TIME_CHANNEL_OUTBOUND_LIQUIDITY_SAT),
    )
    .await
    .unwrap();

    close_channel(
        TestPathChannelClose::Force,
        &payee,
        &coordinator,
        invoice_amount,
    )
    .await
    .unwrap();
}

async fn close_channel(
    test_path: TestPathChannelClose,
    payee: &Node<PaymentMap>,
    coordinator: &Node<PaymentMap>,
    lightning_balance: u64,
) -> Result<()> {
    let channel_id = payee
        .channel_manager
        .list_usable_channels()
        .first()
        .unwrap()
        .channel_id;

    assert_eq!(payee.get_on_chain_balance()?.confirmed, 0);
    assert_eq!(payee.get_ldk_balance().available, lightning_balance);
    assert_eq!(payee.get_ldk_balance().pending_close, 0);

    match test_path {
        TestPathChannelClose::Force => {
            payee
                .channel_manager
                .force_close_channel(&channel_id, &coordinator.info.pubkey)
                .map_err(|e| anyhow!("{e:?}"))?;
        }
        TestPathChannelClose::Collaborative => {
            payee
                .channel_manager
                .close_channel(&channel_id, &coordinator.info.pubkey)
                .map_err(|e| anyhow!("{e:?}"))?;

            // wait for the collaboration closure to complete.
            // todo: it would be nice if we could simply assert the channel close event.
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }

    payee.sync()?;

    assert_eq!(payee.get_on_chain_balance()?.confirmed, 0);
    assert_eq!(payee.get_ldk_balance().available, 0);
    assert_eq!(payee.get_ldk_balance().pending_close, lightning_balance);

    // transaction fees for the on-chain transaction
    let tx_fees = match test_path {
        TestPathChannelClose::Force => {
            // the delay we have to wait before the fund can be claimed on chain again.
            bitcoind::mine(144).await?;
            122
        }
        TestPathChannelClose::Collaborative => {
            // mine six block after broadcasting the commit transaction.
            bitcoind::mine(6).await?;
            110
        }
    };

    // this sync triggers the `[Event::SpendableOutputs]` broadcasting the transaction to claim
    // the payees coins.
    payee.sync()?;

    // mine a single block to claim the spendable output after waiting for the force close delay.
    bitcoind::mine(1).await?;
    payee.sync()?;

    assert_eq!(payee.get_ldk_balance().available, 0);
    assert_eq!(payee.get_ldk_balance().pending_close, 0);
    // the confirmed balance is greater 0. No exact match as the fee rates vary the result.
    assert_eq!(
        payee.get_on_chain_balance()?.confirmed,
        lightning_balance - tx_fees
    );

    Ok(())
}
