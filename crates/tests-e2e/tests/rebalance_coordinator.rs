use anyhow::Result;
use bitcoin::Amount;
use std::time::Duration;
use tests_e2e::bitcoind::Bitcoind;
use tests_e2e::coordinator::Coordinator;
use tests_e2e::fund::open_channel;
use tests_e2e::fund::pay_with_faucet;
use tests_e2e::http::init_reqwest;
use tests_e2e::logger::init_tracing;
use tests_e2e::maker::Maker;
use tests_e2e::wait_until;

#[tokio::test]
#[ignore = "need to be run with 'just e2e' command"]
async fn coordinator_can_rebalance_channel() -> Result<()> {
    init_tracing();

    let client = init_reqwest();
    let bitcoind = Bitcoind::new_local(client.clone());

    let coordinator = Coordinator::new_local(client.clone());
    assert!(coordinator.is_running().await);

    tracing::info!("coordinator up");

    // Start maker after coordinator as its health check needs coordinator
    let maker = Maker::new_local(client.clone());
    wait_until!(maker.is_running().await);
    tracing::info!("maker up");

    let node_info_maker = maker.get_node_info().await?;

    // // Ensure the maker has a free UTXO available.
    // let address = maker.get_new_address().await.unwrap();
    // bitcoind
    //     .send_to_address(&address, Amount::ONE_BTC)
    //     .await
    //     .unwrap();
    // bitcoind.mine(1).await.unwrap();
    // maker.sync_on_chain().await.unwrap();

    // LND to open channel with maker
    let outbound_liquidity_maker = 50_000_000;

    // open_channel(
    //     &node_info_maker,
    //     Amount::from_sat(outbound_liquidity_maker),
    //     "http://localhost:8080",
    //     &bitcoind,
    // )
    // .await?;
    // TODO: maker to do node announcement
    // maker.broadcast_node_announcement().await.unwrap();

    tracing::info!("lnd channel open to maker");

    // let invoice = maker
    //     .create_invoice(Some(outbound_liquidity_maker / 2))
    //     .await
    //     .unwrap();

    // let maker_balance_before_invoice = maker.get_balance().await.unwrap();

    // pay_with_faucet(&client, invoice.to_string()).await.unwrap();

    // maker.sync_on_chain().await.unwrap();

    // let maker_balance_after_invoice = maker.get_balance().await.unwrap();

    // assert!(
    //     dbg!(maker_balance_before_invoice.offchain) < dbg!(maker_balance_after_invoice.offchain)
    // );

    // Coordinator to open channel with maker
    // let outbound_liquidity = 12_345_000;

    // let balance_coordinator_before_channel = coordinator.get_balance().await?.offchain;
    // dbg!(&balance_coordinator_before_channel);

    // coordinator
    //     .open_channel(node_info_maker, outbound_liquidity, None)
    //     .await?;
    // tracing::info!("coordinator channel opened to maker");

    // Wait for the channels to be open.
    // tokio::time::sleep(Duration::from_secs(5)).await;

    // Mine seven blocks to render the public channel is usable.
    // bitcoind.mine(7).await.unwrap();
    // coordinator.sync_wallet().await.unwrap();

    // let balance_maker_before_channel = maker.get_balance().await?.offchain;
    // dbg!(balance_maker_before_channel);
    // maker.sync_on_chain().await.unwrap();

    tracing::info!("got till here");

    // coordinator.sync_wallet().await.unwrap();
    // let balance_coordinator_after_channel = coordinator.get_balance().await?.offchain;
    // dbg!(&balance_coordinator_after_channel);

    tracing::debug!("Waiting if outbound channel is open and usable");

    wait_until!(coordinator
        .get_channels()
        .await
        .unwrap()
        .iter()
        .any(|channel| {
            channel.is_outbound
                && channel.is_usable
                && channel.counterparty == node_info_maker.pubkey.to_string()
        }));

    tracing::debug!("Looking good");

    // coordinator.sync_wallet().await.unwrap();
    // let balance_after_after_channel = coordinator.get_balance().await?.offchain;

    // assert_eq!(
    //     balance_coordinator_before_channel + outbound_liquidity,
    //     balance_after_after_channel
    // );

    tracing::debug!("Finally starting with rebalance setup");

    /*// get biggest in-bound channel
    let mut all_channels = coordinator.get_channels().await.unwrap();
    all_channels.sort_by(|a, b| a.inbound_capacity_msat.cmp(&b.inbound_capacity_msat));
    // the first item in the list should have 0 in-bound liquidity as it is the just created one
    let outbound_channel = all_channels.get(0).expect("to have at least one item");
    // the last item in the list should have a lot in-bound liquidity as it is the LND channel
    let inbound_channel = all_channels.last().expect("to have a last item");

    dbg!(&inbound_channel);
    dbg!(&outbound_channel);

    // act: rebalance
    coordinator
        .rebalance(
            10_000,
            &outbound_channel.channel_id,
            &inbound_channel.channel_id,
        )
        .await
        .unwrap();

    coordinator.sync_wallet().await.unwrap();

    // Wait for the payment to be settled
    tokio::time::sleep(Duration::from_secs(5)).await;

    let all_channels = coordinator.get_channels().await.unwrap();
    let inbound_channel = all_channels
        .iter()
        .find(|c| c.channel_id == inbound_channel.channel_id)
        .expect("Channel to not disappear");

    dbg!(inbound_channel);*/

    Ok(())
}
