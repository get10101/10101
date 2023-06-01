use crate::disk;
use crate::dlc_custom_signer::CustomKeysManager;
use crate::fee_rate_estimator::FeeRateEstimator;
use crate::ln::app_config;
use crate::ln::coordinator_config;
use crate::ln::EventHandler;
use crate::ln::TracingLogger;
use crate::ln_dlc_wallet::LnDlcWallet;
use crate::node::dlc_channel::process_pending_dlc_actions;
use crate::node::peer_manager::broadcast_node_announcement;
use crate::on_chain_wallet::OnChainWallet;
use crate::seed::Bip39Seed;
use crate::util;
use crate::ChainMonitor;
use crate::FakeChannelPaymentRequests;
use crate::NetworkGraph;
use crate::PeerManager;
use anyhow::ensure;
use anyhow::Context;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use bitcoin::Network;
use dlc_messages::message_handler::MessageHandler as DlcMessageHandler;
use dlc_sled_storage_provider::SledStorageProvider;
use futures::future::RemoteHandle;
use futures::FutureExt;
use lightning::chain::chainmonitor;
use lightning::chain::keysinterface::EntropySource;
use lightning::chain::keysinterface::KeysManager;
use lightning::chain::Confirm;
use lightning::ln::msgs::NetAddress;
use lightning::ln::peer_handler::IgnoringMessageHandler;
use lightning::ln::peer_handler::MessageHandler;
use lightning::routing::gossip::P2PGossipSync;
use lightning::routing::router::DefaultRouter;
use lightning::routing::utxo::UtxoLookup;
use lightning::util::config::UserConfig;
use lightning_background_processor::process_events_async;
use lightning_background_processor::GossipSync;
use lightning_persister::FilesystemPersister;
use lightning_transaction_sync::EsploraSyncClient;
use p2pd_oracle_client::P2PDOracleClient;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;
use tokio::task::spawn_blocking;

mod channel_manager;
mod connection;
pub(crate) mod dlc_channel;
mod dlc_manager;
pub(crate) mod invoice;
mod ln_channel;
mod oracle_client;
mod payment_persister;
mod peer_manager;
mod sub_channel_manager;
mod wallet;

pub use self::dlc_manager::DlcManager;
pub use ::dlc_manager as rust_dlc_manager;
pub use channel_manager::ChannelManager;
pub use dlc_channel::dlc_message_name;
pub use dlc_channel::sub_channel_message_name;
pub use invoice::HTLCStatus;
pub use payment_persister::PaymentMap;
pub use payment_persister::PaymentPersister;
pub use sub_channel_manager::SubChannelManager;
pub use wallet::PaymentDetails;

/// The interval at which the [`lightning::ln::msgs::NodeAnnouncement`] is broadcast.
///
/// According to the LDK team, a value of up to 1 hour should be fine.
const BROADCAST_NODE_ANNOUNCEMENT_INTERVAL: Duration = Duration::from_secs(600);

/// An LN-DLC node.
pub struct Node<P> {
    pub settings: Arc<RwLock<LnDlcNodeSettings>>,
    pub network: Network,

    pub(crate) wallet: Arc<LnDlcWallet>,

    pub peer_manager: Arc<PeerManager>,
    pub channel_manager: Arc<ChannelManager>,
    chain_monitor: Arc<ChainMonitor>,
    keys_manager: Arc<CustomKeysManager>,
    pub network_graph: Arc<NetworkGraph>,
    pub fee_rate_estimator: Arc<FeeRateEstimator>,
    _background_processor_handle: RemoteHandle<()>,
    _connection_manager_handle: RemoteHandle<()>,
    _broadcast_node_announcement_handle: RemoteHandle<()>,
    _pending_dlc_actions_handle: RemoteHandle<()>,

    logger: Arc<TracingLogger>,

    pub info: NodeInfo,
    fake_channel_payments: FakeChannelPaymentRequests,

    pub dlc_manager: Arc<DlcManager>,
    pub sub_channel_manager: Arc<SubChannelManager>,
    oracle: Arc<P2PDOracleClient>,
    pub dlc_message_handler: Arc<DlcMessageHandler>,
    payment_persister: Arc<P>,
    pub(crate) user_config: UserConfig,
    #[cfg(test)]
    pub(crate) alias: [u8; 32],
    #[cfg(test)]
    pub(crate) announcement_addresses: Vec<NetAddress>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct NodeInfo {
    pub pubkey: PublicKey,
    pub address: SocketAddr,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LnDlcNodeSettings {
    /// How often we sync the LDK wallet
    pub off_chain_sync_interval: Duration,
    /// How often we sync the BDK wallet
    pub on_chain_sync_interval: Duration,
    /// How often we update the fee rate
    pub fee_rate_sync_interval: Duration,

    /// The 'stop gap' parameter used by BDK's wallet sync. This seems to configure the threshold
    /// number of blocks after which BDK stops looking for scripts belonging to the wallet.
    /// Note: This constant and value was copied from ldk_node
    /// XXX: Requires restart of the node to take effect
    pub bdk_client_stop_gap: usize,
    /// The number of concurrent requests made against the API provider.
    /// Note: This constant and value was copied from ldk_node
    /// XXX: Requires restart of the node to take effect
    pub bdk_client_concurrency: u8,
}

impl Default for LnDlcNodeSettings {
    fn default() -> Self {
        Self {
            off_chain_sync_interval: Duration::from_secs(5),
            on_chain_sync_interval: Duration::from_secs(300),
            fee_rate_sync_interval: Duration::from_secs(20),
            bdk_client_stop_gap: 20,
            bdk_client_concurrency: 4,
        }
    }
}

impl<P> Node<P>
where
    P: PaymentPersister + Send + Sync + 'static,
{
    /// Constructs a new node to be run as the app
    #[allow(clippy::too_many_arguments)]
    pub fn new_app(
        alias: &str,
        network: Network,
        data_dir: &Path,
        payment_persister: P,
        announcement_address: SocketAddr,
        listen_address: SocketAddr,
        esplora_server_url: String,
        seed: Bip39Seed,
        ephemeral_randomness: [u8; 32],
    ) -> Result<Self> {
        let user_config = app_config();
        Node::new(
            alias,
            network,
            data_dir,
            payment_persister,
            announcement_address,
            listen_address,
            vec![util::build_net_address(
                announcement_address.ip(),
                announcement_address.port(),
            )],
            esplora_server_url,
            seed,
            ephemeral_randomness,
            user_config,
            LnDlcNodeSettings::default(),
        )
    }

    /// Constructs a new node to be run for the coordinator
    ///
    /// The main difference between this and `new_app` is that the user config is different to
    /// be able to create just-in-time channels and 0-conf channels towards our peers.
    #[allow(clippy::too_many_arguments)]
    pub fn new_coordinator(
        alias: &str,
        network: Network,
        data_dir: &Path,
        payment_persister: P,
        announcement_address: SocketAddr,
        listen_address: SocketAddr,
        announcement_addresses: Vec<NetAddress>,
        esplora_server_url: String,
        seed: Bip39Seed,
        ephemeral_randomness: [u8; 32],
        settings: LnDlcNodeSettings,
    ) -> Result<Self> {
        let mut user_config = coordinator_config();

        // TODO: The config `force_announced_channel_preference` has been temporarily disabled
        // for testing purposes, as otherwise the app is not able to open a channel to the
        // coordinator. Remove this config, once not needed anymore.
        user_config
            .channel_handshake_limits
            .force_announced_channel_preference = false;
        Self::new(
            alias,
            network,
            data_dir,
            payment_persister,
            announcement_address,
            listen_address,
            announcement_addresses,
            esplora_server_url,
            seed,
            ephemeral_randomness,
            user_config,
            settings,
        )
    }

    pub fn update_settings(&self, new_settings: LnDlcNodeSettings) {
        tracing::info!(?new_settings, "Updating LnDlcNode settings");
        *self.settings.write().expect("RwLock to not be poisoned") = new_settings;
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        alias: &str,
        network: Network,
        data_dir: &Path,
        payment_persister: P,
        announcement_address: SocketAddr,
        listen_address: SocketAddr,
        announcement_addresses: Vec<NetAddress>,
        esplora_server_url: String,
        seed: Bip39Seed,
        ephemeral_randomness: [u8; 32],
        ldk_user_config: UserConfig,
        settings: LnDlcNodeSettings,
    ) -> Result<Self> {
        let time_since_unix_epoch = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?;

        let logger = Arc::new(TracingLogger);

        if !data_dir.exists() {
            std::fs::create_dir_all(data_dir)
                .context(format!("Could not create data dir ({data_dir:?})"))?;
        }

        let ldk_data_dir = data_dir.to_string_lossy().to_string();
        let persister = Arc::new(FilesystemPersister::new(ldk_data_dir.clone()));

        let storage = Arc::new(SledStorageProvider::new(
            data_dir.to_str().expect("data_dir"),
        )?);

        let on_chain_dir = data_dir.join("on_chain");
        let on_chain_wallet =
            OnChainWallet::new(on_chain_dir.as_path(), network, seed.wallet_seed())?;

        let esplora_client = Arc::new(EsploraSyncClient::new(
            esplora_server_url.clone(),
            logger.clone(),
        ));

        let fee_rate_estimator = Arc::new(FeeRateEstimator::new(esplora_server_url));
        let ln_dlc_wallet = {
            Arc::new(LnDlcWallet::new(
                esplora_client.clone(),
                on_chain_wallet.inner,
                fee_rate_estimator.clone(),
                storage.clone(),
                seed.clone(),
                settings.bdk_client_stop_gap,
                settings.bdk_client_concurrency,
            ))
        };

        let settings = Arc::new(RwLock::new(settings));

        std::thread::spawn({
            let settings = settings.clone();
            let ln_dlc_wallet = ln_dlc_wallet.clone();
            move || {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("to be able to create a runtime")
                    .block_on(async move {
                        loop {
                            let now = Instant::now();
                            match ln_dlc_wallet.inner().sync().await {
                                Ok(()) => tracing::info!(
                                    "Background sync of on-chain wallet finished in {}ms.",
                                    now.elapsed().as_millis()
                                ),
                                Err(err) => {
                                    tracing::error!(
                                        "Background sync of on-chain wallet failed: {err:#}",
                                    )
                                }
                            }
                            tokio::time::sleep(
                                settings
                                    .read()
                                    .expect("RwLock to not be poisoned")
                                    .on_chain_sync_interval,
                            )
                            .await;
                        }
                    });
            }
        });

        tokio::spawn({
            let settings = settings.clone();
            let fee_rate_estimator = fee_rate_estimator.clone();
            async move {
                loop {
                    let interval = settings
                        .read()
                        .expect("RwLock to not be poisoned")
                        .fee_rate_sync_interval;

                    if let Err(err) = fee_rate_estimator.update().await {
                        tracing::error!("Failed to update fee rate estimates: {err:#}");
                    }

                    tokio::time::sleep(interval).await;
                }
            }
        });

        let chain_monitor: Arc<ChainMonitor> = Arc::new(chainmonitor::ChainMonitor::new(
            Some(esplora_client.clone()),
            ln_dlc_wallet.clone(),
            logger.clone(),
            fee_rate_estimator.clone(),
            persister.clone(),
        ));

        let keys_manager = {
            Arc::new(CustomKeysManager::new(
                KeysManager::new(
                    &seed.lightning_seed(),
                    time_since_unix_epoch.as_secs(),
                    time_since_unix_epoch.subsec_nanos(),
                ),
                ln_dlc_wallet.clone(),
            ))
        };

        let network_graph_path = format!("{ldk_data_dir}/network_graph");
        let network_graph = Arc::new(disk::read_network(
            Path::new(&network_graph_path),
            network,
            logger.clone(),
        ));

        let scorer_path = data_dir.join("scorer");
        let scorer = Arc::new(Mutex::new(disk::read_scorer(
            scorer_path.as_path(),
            network_graph.clone(),
            logger.clone(),
        )));

        let router = Arc::new(DefaultRouter::new(
            network_graph.clone(),
            logger.clone(),
            keys_manager.get_secure_random_bytes(),
            scorer.clone(),
        ));

        let channel_manager = channel_manager::build(
            &ldk_data_dir,
            keys_manager.clone(),
            ln_dlc_wallet.clone(),
            fee_rate_estimator.clone(),
            esplora_client.clone(),
            logger.clone(),
            chain_monitor.clone(),
            ldk_user_config,
            network,
            persister.clone(),
            router,
        )?;

        let channel_manager = Arc::new(channel_manager);

        tokio::spawn({
            let channel_manager = channel_manager.clone();
            let chain_monitor = chain_monitor.clone();
            let settings = settings.clone();
            async move {
                loop {
                    let interval = settings
                        .read()
                        .expect("RwLock to not be poisoned")
                        .off_chain_sync_interval;

                    let now = Instant::now();
                    let confirmables = vec![
                        &*channel_manager as &(dyn Confirm + Sync + Send),
                        &*chain_monitor as &(dyn Confirm + Sync + Send),
                    ];
                    match esplora_client.sync(confirmables) {
                        Ok(()) => tracing::info!(
                            "Background sync of Lightning wallet finished in {}ms.",
                            now.elapsed().as_millis()
                        ),
                        Err(e) => {
                            tracing::error!("Background sync of Lightning wallet failed: {e:#}")
                        }
                    }
                    tokio::time::sleep(interval).await;
                }
            }
        });

        let gossip_sync = Arc::new(P2PGossipSync::new(
            network_graph.clone(),
            None::<Arc<dyn UtxoLookup + Send + Sync>>,
            logger.clone(),
        ));

        let oracle_client = oracle_client::build();
        let oracle_client = Arc::new(oracle_client);

        let dlc_manager = dlc_manager::build(
            data_dir,
            ln_dlc_wallet.clone(),
            storage,
            oracle_client.clone(),
            fee_rate_estimator.clone(),
        )?;
        let dlc_manager = Arc::new(dlc_manager);

        let sub_channel_manager =
            sub_channel_manager::build(channel_manager.clone(), dlc_manager.clone())?;

        let lightning_msg_handler = MessageHandler {
            chan_handler: sub_channel_manager.clone(),
            route_handler: gossip_sync.clone(),
            onion_message_handler: Arc::new(IgnoringMessageHandler {}),
        };

        let dlc_message_handler = Arc::new(DlcMessageHandler::new());

        let peer_manager: Arc<PeerManager> = Arc::new(PeerManager::new(
            lightning_msg_handler,
            time_since_unix_epoch.as_secs() as u32,
            &ephemeral_randomness,
            logger.clone(),
            dlc_message_handler.clone(),
            keys_manager.clone(),
        ));

        let fake_channel_payments: FakeChannelPaymentRequests =
            Arc::new(Mutex::new(HashMap::new()));

        let payment_persister = Arc::new(payment_persister);
        let event_handler = EventHandler::new(
            channel_manager.clone(),
            ln_dlc_wallet.clone(),
            network_graph.clone(),
            keys_manager.clone(),
            payment_persister.clone(),
            fake_channel_payments.clone(),
            Arc::new(Mutex::new(HashMap::new())),
            peer_manager.clone(),
            fee_rate_estimator.clone(),
        );

        // Connection manager
        let connection_manager_handle = {
            let peer_manager = peer_manager.clone();
            let (fut, remote_handle) = async move {
                let mut connection_handles = Vec::new();

                let listener = tokio::net::TcpListener::bind(listen_address)
                    .await
                    .expect("Failed to bind to listen port");
                loop {
                    let peer_manager = peer_manager.clone();
                    let (tcp_stream, addr) = match listener.accept().await {
                        Ok(ret) => ret,
                        Err(e) => {
                            tracing::error!("Failed to accept incoming connection: {e:#}");
                            continue;
                        }
                    };

                    tracing::debug!(%addr, "Received inbound connection");

                    let (fut, connection_handle) = async move {
                        lightning_net_tokio::setup_inbound(
                            peer_manager.clone(),
                            tcp_stream.into_std().expect("Stream conversion to succeed"),
                        )
                        .await;
                    }
                    .remote_handle();

                    connection_handles.push(connection_handle);

                    tokio::spawn(fut);
                }
            }
            .remote_handle();

            tokio::spawn(fut);

            remote_handle
        };

        tracing::info!("Listening on {listen_address}");

        tracing::info!("Starting background processor");

        let background_processor_handle = {
            let peer_manager = peer_manager.clone();
            let channel_manager = channel_manager.clone();
            let chain_monitor = chain_monitor.clone();
            let logger = logger.clone();

            let (fut, remote_handle) = async move {
                if let Err(e) = process_events_async(
                    persister,
                    |e| event_handler.handle_event(e),
                    chain_monitor,
                    channel_manager,
                    GossipSync::p2p(gossip_sync),
                    peer_manager,
                    logger,
                    Some(scorer),
                    |d| {
                        Box::pin(async move {
                            tokio::time::sleep(d).await;
                            false
                        })
                    },
                )
                .await
                {
                    tracing::error!("Error running background processor: {e}");
                }
            }
            .remote_handle();

            tokio::spawn(fut);

            remote_handle
        };

        let alias = alias_as_bytes(alias)?;
        let node_announcement_interval = node_announcement_interval(network);
        let broadcast_node_announcement_handle = {
            #[cfg(test)]
            let announcement_addresses = announcement_addresses.clone();
            #[cfg(not(test))]
            let announcement_addresses = announcement_addresses;
            let peer_manager = peer_manager.clone();
            let (fut, remote_handle) = async move {
                let mut interval = tokio::time::interval(node_announcement_interval);
                loop {
                    broadcast_node_announcement(
                        &peer_manager,
                        alias,
                        announcement_addresses.clone(),
                    );

                    interval.tick().await;
                }
            }
            .remote_handle();

            tokio::spawn(fut);

            remote_handle
        };

        let pending_dlc_actions_handle = {
            let sub_channel_manager = sub_channel_manager.clone();
            let dlc_message_handler = dlc_message_handler.clone();
            let (fut, remote_handle) = {
                async move {
                    loop {
                        if let Err(e) = spawn_blocking({
                            let sub_channel_manager = sub_channel_manager.clone();
                            let dlc_message_handler = dlc_message_handler.clone();
                            move || {
                                process_pending_dlc_actions(
                                    sub_channel_manager.clone(),
                                    &dlc_message_handler,
                                )
                            }
                        })
                        .await
                        {
                            tracing::error!("Failed to process pending DLC actions: {e:#}");
                        };

                        tokio::time::sleep(Duration::from_secs(30)).await;
                    }
                }
            }
            .remote_handle();

            tokio::spawn(fut);

            remote_handle
        };

        tokio::task::spawn({
            let dlc_manager = dlc_manager.clone();
            async move {
                loop {
                    if let Err(e) = spawn_blocking({
                        let dlc_manager = dlc_manager.clone();
                        move || dlc_manager.periodic_check()
                    })
                    .await
                    .expect("task to complete")
                    {
                        tracing::error!("Failed DLC manager periodic check: {e:#}");
                    }

                    tokio::time::sleep(Duration::from_secs(30)).await;
                }
            }
        });

        let node_info = NodeInfo {
            pubkey: channel_manager.get_our_node_id(),
            address: announcement_address,
        };

        tracing::info!("Lightning node started with node ID {}", node_info);

        Ok(Self {
            network,
            wallet: ln_dlc_wallet,
            peer_manager,
            keys_manager,
            chain_monitor,
            logger,
            channel_manager: channel_manager.clone(),
            info: node_info,
            fake_channel_payments,
            sub_channel_manager,
            oracle: oracle_client,
            dlc_message_handler,
            dlc_manager,
            payment_persister,
            fee_rate_estimator,
            user_config: ldk_user_config,
            _background_processor_handle: background_processor_handle,
            _connection_manager_handle: connection_manager_handle,
            _broadcast_node_announcement_handle: broadcast_node_announcement_handle,
            _pending_dlc_actions_handle: pending_dlc_actions_handle,
            network_graph,
            #[cfg(test)]
            announcement_addresses,
            #[cfg(test)]
            alias,
            settings,
        })
    }
}

impl Display for NodeInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        format!("{}@{}", self.pubkey, self.address).fmt(f)
    }
}

fn alias_as_bytes(alias: &str) -> Result<[u8; 32]> {
    ensure!(
        alias.len() <= 32,
        "Node Alias can not be longer than 32 bytes"
    );

    let mut bytes = [0; 32];
    bytes[..alias.len()].copy_from_slice(alias.as_bytes());

    Ok(bytes)
}

fn node_announcement_interval(network: Network) -> Duration {
    match network {
        // We want to broadcast node announcements more frequently on regtest to make testing easier
        Network::Regtest => Duration::from_secs(30),
        _ => BROADCAST_NODE_ANNOUNCEMENT_INTERVAL,
    }
}
