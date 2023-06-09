use crate::ln::app_config;
use crate::node::Node;
use crate::node::NodeInfo;
use crate::node::PaymentMap;
use crate::tests::init_tracing;
use crate::tests::wait_until;
use anyhow::Result;
use coordinator::Coordinator;
use coordinator::Direction;
use dlc_manager::subchannel::SubChannelState;
use dlc_manager::Storage;
use std::borrow::Borrow;
use std::sync::Arc;
use std::time::Duration;

mod coordinator;

const ESPLORA_ORIGIN_PUBLIC_REGTEST: &str = "http://35.189.57.114:3000";

#[tokio::test]
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

async fn open_position(coordinator: &Coordinator, app: &Node<PaymentMap>) -> Result<()> {
    tracing::info!("Opening position");

    loop {
        tracing::info!("Sending open pre-proposal");

        if coordinator.post_trade(app, Direction::Long).await.is_ok() {
            break;
        }

        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    tracing::info!("Open pre-proposal delivered");

    let channel_id = wait_until(Duration::from_secs(60), || async {
        tracing::info!("Waiting for DLC channel proposal");

        app.process_incoming_messages()?;

        let dlc_channel = app
            .dlc_manager
            .get_store()
            .get_sub_channels()?
            .first()
            .cloned();

        Ok(match dlc_channel {
            Some(dlc_channel) if matches!(dlc_channel.state, SubChannelState::Offered(_)) => {
                Some(dlc_channel.channel_id)
            }
            _ => None,
        })
    })
    .await?;

    app.accept_dlc_channel_offer(&channel_id)?;

    wait_until(Duration::from_secs(60), || async {
        tracing::info!("Waiting for Signed state");

        app.process_incoming_messages()?;

        let dlc_channel = app
            .dlc_manager
            .get_store()
            .get_sub_channels()?
            .into_iter()
            .find(|sc| sc.channel_id == channel_id)
            .unwrap();

        Ok(matches!(dlc_channel.state, SubChannelState::Signed(_)).then_some(()))
    })
    .await?;

    tracing::info!("Position open");

    Ok(())
}

async fn close_position(coordinator: &Coordinator, app: &Node<PaymentMap>) -> Result<()> {
    tracing::info!("Closing position");

    loop {
        tracing::info!("Sending close pre-proposal");

        if coordinator.post_trade(app, Direction::Short).await.is_ok() {
            break;
        }

        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    tracing::info!("Close pre-proposal delivered");

    let channel_id = wait_until(Duration::from_secs(60), || async {
        tracing::info!("Waiting for DLC channel close proposal");

        // Process confirm message and send finalize message
        app.process_incoming_messages()?;

        let dlc_channel = app
            .dlc_manager
            .get_store()
            .get_sub_channels()?
            .first()
            .cloned();

        Ok(match dlc_channel {
            Some(dlc_channel) if matches!(dlc_channel.state, SubChannelState::CloseOffered(_)) => {
                Some(dlc_channel.channel_id)
            }
            _ => None,
        })
    })
    .await?;

    app.accept_dlc_channel_collaborative_settlement(&channel_id)
        .unwrap();

    wait_until(Duration::from_secs(60), || async {
        tracing::info!("Waiting for OffChainClosed state");

        // Process confirm message and send finalize message
        app.process_incoming_messages()?;

        let dlc_channel = app
            .dlc_manager
            .get_store()
            .get_sub_channels()?
            .into_iter()
            .find(|sc| sc.channel_id == channel_id)
            .unwrap();

        Ok(matches!(dlc_channel.state, SubChannelState::OffChainClosed).then_some(()))
    })
    .await?;

    tracing::info!("Position closed");

    Ok(())
}

async fn keep_connected(node: impl Borrow<Node<PaymentMap>>, peer: NodeInfo) {
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
