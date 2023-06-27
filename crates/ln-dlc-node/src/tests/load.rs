use crate::ln::app_config;
use crate::node::InMemoryStore;
use crate::node::Node;
use crate::node::NodeInfo;
use crate::tests::init_tracing;
use crate::tests::wait_until_dlc_channel_state;
use crate::tests::SubChannelStateName;
use anyhow::Result;
use coordinator::Coordinator;
use coordinator::Direction;
use std::borrow::Borrow;
use std::sync::Arc;
use std::time::Duration;

mod coordinator;

const ESPLORA_ORIGIN_PUBLIC_REGTEST: &str = "http://35.189.57.114:3000";

#[tokio::test(flavor = "multi_thread")]
async fn single_app_many_positions_load() {
    init_tracing();

    let coordinator = Coordinator::new_public_regtest();
    let app = Arc::new(
        Node::start_test(
            "app",
            app_config(),
            ESPLORA_ORIGIN_PUBLIC_REGTEST.to_string(),
        )
        .unwrap(),
    );

    tokio::spawn({
        let app = app.clone();
        let coordinator_info = coordinator.info();
        async move { keep_connected(app, coordinator_info).await }
    });

    tokio::time::sleep(Duration::from_secs(5)).await;

    // Operating the bitcoin node remotely is too much of a hassle. Just prepare the environment
    // before running this test
    coordinator
        .open_channel(&app, 200_000, 100_000)
        .await
        .unwrap();

    for n in 1..100 {
        tracing::info!(%n, "Starting iteration");

        open_position(&coordinator, &app).await.unwrap();
        close_position(&coordinator, &app).await.unwrap();

        tracing::info!(%n, "Finished iteration");
    }
}

async fn open_position(coordinator: &Coordinator, app: &Node<InMemoryStore>) -> Result<()> {
    tracing::info!("Opening position");

    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            tracing::info!("Sending open pre-proposal");

            match coordinator.post_trade(app, Direction::Long).await {
                Ok(_) => break,
                Err(e) => {
                    tracing::debug!("Could not yet process open pre-proposal: {e:#}");
                }
            }

            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    })
    .await
    .unwrap();

    tracing::info!("Open pre-proposal delivered");

    let dlc_channel = wait_until_dlc_channel_state(
        Duration::from_secs(60),
        app,
        coordinator.info().pubkey,
        SubChannelStateName::Offered,
    )
    .await?;

    app.accept_dlc_channel_offer(&dlc_channel.channel_id)?;

    wait_until_dlc_channel_state(
        Duration::from_secs(60),
        app,
        coordinator.info().pubkey,
        SubChannelStateName::Signed,
    )
    .await?;

    tracing::info!("Position open");

    Ok(())
}

async fn close_position(coordinator: &Coordinator, app: &Node<InMemoryStore>) -> Result<()> {
    tracing::info!("Closing position");

    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            tracing::info!("Sending close pre-proposal");

            match coordinator.post_trade(app, Direction::Short).await {
                Ok(_) => break,
                Err(e) => {
                    tracing::debug!("Could not yet process close pre-proposal: {e:#}");
                }
            }

            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    })
    .await
    .unwrap();

    tracing::info!("Close pre-proposal delivered");

    let dlc_channel = wait_until_dlc_channel_state(
        Duration::from_secs(60),
        app,
        coordinator.info().pubkey,
        SubChannelStateName::CloseOffered,
    )
    .await?;

    app.accept_dlc_channel_collaborative_settlement(&dlc_channel.channel_id)
        .unwrap();

    wait_until_dlc_channel_state(
        Duration::from_secs(60),
        app,
        coordinator.info().pubkey,
        SubChannelStateName::OffChainClosed,
    )
    .await?;

    tracing::info!("Position closed");

    Ok(())
}

async fn keep_connected(node: impl Borrow<Node<InMemoryStore>>, peer: NodeInfo) {
    let reconnect_interval = Duration::from_secs(1);
    loop {
        let connection_closed_future = match node.borrow().connect(peer).await {
            Ok(fut) => fut,
            Err(e) => {
                tracing::warn!(
                    %peer,
                    ?reconnect_interval,
                    "Connection failed: {e:#}; reconnecting"
                );

                tokio::time::sleep(reconnect_interval).await;
                continue;
            }
        };

        connection_closed_future.await;
        tracing::debug!(
            %peer,
            ?reconnect_interval,
            "Connection lost; reconnecting"
        );

        tokio::time::sleep(reconnect_interval).await;
    }
}
