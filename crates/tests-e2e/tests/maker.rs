use anyhow::Result;
use bitcoin::Amount;
use std::time::Duration;
use tests_e2e::bitcoind::Bitcoind;
use tests_e2e::coordinator::Coordinator;
use tests_e2e::http::init_reqwest;
use tests_e2e::logger::init_tracing;
use tests_e2e::maker::Maker;
use tests_e2e::wait_until;

#[tokio::test]
#[ignore = "need to be run with 'just e2e' command"]
async fn maker_can_open_channel_to_coordinator_and_send_payment() -> Result<()> {
    init_tracing();

    let client = init_reqwest();

    let maker = Maker::new_local(client.clone());
    assert!(maker.is_running().await);

    let coordinator = Coordinator::new_local(client.clone());
    assert!(coordinator.is_running().await);

    let node_info_coordinator = coordinator.get_node_info().await?;

    // Ensure the maker has a free UTXO available.
    let address = maker.get_new_address().await.unwrap();
    let bitcoind = Bitcoind::new(client.clone());
    bitcoind
        .send_to_address(address, Amount::ONE_BTC)
        .await
        .unwrap();
    bitcoind.mine(101).await.unwrap();
    maker.sync_on_chain().await.unwrap();

    let maker_on_chain_balance = maker.get_balance().await.unwrap().onchain;
    assert!(maker_on_chain_balance > 0);

    let balance_maker_before_channel = maker.get_balance().await?.offchain;

    let outbound_liquidity_maker = 500_000;
    maker
        .open_channel(node_info_coordinator, outbound_liquidity_maker, None)
        .await?;

    // Wait for the channel between maker and coordinator to be open.
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Mine one block to render the public channel is usable.
    bitcoind.mine(1).await.unwrap();
    coordinator.sync_wallet().await.unwrap();
    maker.sync_on_chain().await.unwrap();

    let balance_maker_after_channel = maker.get_balance().await?.offchain;

    assert_eq!(
        balance_maker_before_channel + outbound_liquidity_maker,
        balance_maker_after_channel
    );

    let balance_coordinator_after_channel = coordinator.get_balance().await?.offchain;

    let payment_amount = 100_000;
    let invoice = coordinator.create_invoice(Some(payment_amount)).await?;

    maker.pay_invoice(invoice).await?;

    wait_until!(
        coordinator.get_balance().await.unwrap().offchain > balance_coordinator_after_channel
    );

    let balance_maker_after_payment = maker.get_balance().await?.offchain;
    let balance_coordinator_after_payment = coordinator.get_balance().await?.offchain;

    assert_eq!(
        balance_maker_after_channel - payment_amount,
        balance_maker_after_payment
    );

    assert_eq!(
        balance_coordinator_after_channel + payment_amount,
        balance_coordinator_after_payment
    );

    Ok(())
}
