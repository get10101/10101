use crate::config::app_config;
use crate::node::GossipSourceConfig;
use crate::node::InMemoryStore;
use crate::node::LnDlcNodeSettings;
use crate::node::Node;
use crate::node::NodeInfo;
use crate::node::OracleInfo;
use crate::storage::TenTenOneInMemoryStorage;
use crate::tests::init_tracing;
use crate::tests::wait_until_dlc_channel_state;
use crate::tests::SubChannelStateName;
use crate::AppEventHandler;
use crate::EventHandlerTrait;
use anyhow::Result;
use bitcoin::XOnlyPublicKey;
use coordinator::Coordinator;
use coordinator::Direction;
use std::borrow::Borrow;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

mod coordinator;

const ESPLORA_ORIGIN_PUBLIC_REGTEST: &str = "http://34.32.0.52:3000";
const ORACLE_ORIGIN_PUBLIC_REGTEST: &str = "http://34.32.0.52:8081";
const ORACLE_PUBKEY_PUBLIC_REGTEST: &str =
    "5d12d79f575b8d99523797c46441c0549eb0defb6195fe8a080000cbe3ab3859";

#[tokio::test(flavor = "multi_thread")]
async fn single_app_many_positions_load() {
    init_tracing();

    let coordinator = Coordinator::new_public_regtest();

    let app_event_handler = |node, event_sender| {
        Arc::new(AppEventHandler::new(node, event_sender)) as Arc<dyn EventHandlerTrait>
    };

    let (app, _running_app) = Node::start_test(
        app_event_handler,
        "app",
        app_config(),
        ESPLORA_ORIGIN_PUBLIC_REGTEST.to_string(),
        OracleInfo {
            endpoint: ORACLE_ORIGIN_PUBLIC_REGTEST.to_string(),
            public_key: XOnlyPublicKey::from_str(ORACLE_PUBKEY_PUBLIC_REGTEST).unwrap(),
        },
        Arc::new(InMemoryStore::default()),
        ln_dlc_node_settings(),
        None,
    )
    .unwrap();

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

async fn open_position(
    coordinator: &Coordinator,
    app: &Node<TenTenOneInMemoryStorage, InMemoryStore>,
) -> Result<()> {
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

            tokio::time::sleep(Duration::from_millis(100)).await;
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

    app.accept_sub_channel_offer(&dlc_channel.channel_id)?;

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

async fn close_position(
    coordinator: &Coordinator,
    app: &Node<TenTenOneInMemoryStorage, InMemoryStore>,
) -> Result<()> {
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

            tokio::time::sleep(Duration::from_millis(100)).await;
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

    app.accept_sub_channel_collaborative_settlement(&dlc_channel.channel_id)
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

async fn keep_connected(
    node: impl Borrow<Node<TenTenOneInMemoryStorage, InMemoryStore>>,
    peer: NodeInfo,
) {
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

fn ln_dlc_node_settings() -> LnDlcNodeSettings {
    LnDlcNodeSettings {
        off_chain_sync_interval: Duration::from_secs(5),
        on_chain_sync_interval: Duration::from_secs(300),
        fee_rate_sync_interval: Duration::from_secs(20),
        dlc_manager_periodic_check_interval: Duration::from_secs(30),
        sub_channel_manager_periodic_check_interval: Duration::from_secs(30),
        shadow_sync_interval: Duration::from_secs(600),
        forwarding_fee_proportional_millionths: 50,
        bdk_client_stop_gap: 20,
        bdk_client_concurrency: 4,
        gossip_source_config: GossipSourceConfig::P2pNetwork,
    }
}
