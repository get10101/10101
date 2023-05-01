use crate::disk;
use crate::dlc_custom_signer::CustomKeysManager;
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
use crate::InvoicePayer;
use crate::NetworkGraph;
use crate::PeerManager;
use anyhow::anyhow;
use anyhow::ensure;
use anyhow::Context;
use anyhow::Result;
use bitcoin::blockdata::constants::genesis_block;
use bitcoin::secp256k1::PublicKey;
use bitcoin::Network;
use dlc_messages::message_handler::MessageHandler as DlcMessageHandler;
use dlc_sled_storage_provider::SledStorageProvider;
use futures::future::RemoteHandle;
use futures::FutureExt;
use lightning::chain;
use lightning::chain::chainmonitor;
use lightning::chain::keysinterface::KeysInterface;
use lightning::chain::keysinterface::KeysManager;
use lightning::chain::keysinterface::Recipient;
use lightning::chain::Confirm;
use lightning::ln::msgs::NetAddress;
use lightning::ln::peer_handler::IgnoringMessageHandler;
use lightning::ln::peer_handler::MessageHandler;
use lightning::routing::gossip::P2PGossipSync;
use lightning::routing::router::DefaultRouter;
use lightning::util::config::UserConfig;
use lightning_background_processor::BackgroundProcessor;
use lightning_background_processor::GossipSync;
use lightning_invoice::payment;
use lightning_persister::FilesystemPersister;
use p2pd_oracle_client::P2PDOracleClient;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;

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
use lightning_transaction_sync::EsploraSyncClient;
pub use payment_persister::PaymentMap;
pub use payment_persister::PaymentPersister;
pub use sub_channel_manager::SubChannelManager;
pub use wallet::PaymentDetails;

/// The interval at which the [`lightning::ln::msgs::NodeAnnouncement`] is broadcast.
///
/// According to the LDK team, a value of up to 1 hour should be fine.
const BROADCAST_NODE_ANNOUNCEMENT_INTERVAL: Duration = Duration::from_secs(600);
const OFF_CHAIN_SYNC_INTERVAL: Duration = Duration::from_secs(5);
const ON_CHAIN_SYNC_INTERVAL: Duration = Duration::from_secs(20);

/// An LN-DLC node.
pub struct Node<P> {
    network: Network,

    pub(crate) wallet: Arc<LnDlcWallet>,

    pub peer_manager: Arc<PeerManager>,
    invoice_payer: Arc<InvoicePayer<EventHandler<P>>>,
    pub channel_manager: Arc<ChannelManager>,
    chain_monitor: Arc<ChainMonitor>,
    keys_manager: Arc<CustomKeysManager>,
    pub network_graph: Arc<NetworkGraph>,
    _background_processor: BackgroundProcessor,
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

/// Liquidity-based routing fee in millionths of a routed amount. In
/// other words, 10000 is 1%.
pub(crate) const LIQUIDITY_ROUTING_FEE_MILLIONTHS: u32 = 20_000;

impl<P> Node<P>
where
    P: PaymentPersister + Send + Sync + 'static,
{
    /// Constructs a new node to be run as the app
    #[allow(clippy::too_many_arguments)]
    pub async fn new_app(
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
        )
        .await
    }

    /// Constructs a new node to be run for the coordinator
    ///
    /// The main difference between this and `new_app` is that the user config is different to
    /// be able to create just-in-time channels and 0-conf channels towards our peers.
    #[allow(clippy::too_many_arguments)]
    pub async fn new_coordinator(
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
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn new(
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
    ) -> Result<Self> {
        let time_since_unix_epoch = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?;

        let logger = Arc::new(TracingLogger);

        let stop_running = Arc::new(AtomicBool::new(false));

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

        let tx_sync = Arc::new(EsploraSyncClient::new(
            esplora_server_url,
            Arc::clone(&logger),
        ));

        let ln_dlc_wallet = {
            Arc::new(LnDlcWallet::new(
                tx_sync.clone(),
                on_chain_wallet.inner,
                storage.clone(),
                seed.clone(),
            ))
        };

        let stop_sync = Arc::clone(&stop_running);

        let sync_wallet = ln_dlc_wallet.clone();
        std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async move {
                    loop {
                        if stop_sync.load(Ordering::Acquire) {
                            return;
                        }
                        let now = Instant::now();
                        match sync_wallet.inner().sync().await {
                            Ok(()) => tracing::info!(
                                "Background sync of on-chain wallet finished in {}ms.",
                                now.elapsed().as_millis()
                            ),
                            Err(err) => {
                                tracing::error!(
                                    "Background sync of on-chain wallet failed: {}",
                                    err
                                )
                            }
                        }
                        tokio::time::sleep(ON_CHAIN_SYNC_INTERVAL).await;
                    }
                });
        });

        let chain_monitor: Arc<ChainMonitor> = Arc::new(chainmonitor::ChainMonitor::new(
            Some(tx_sync.clone()),
            ln_dlc_wallet.clone(),
            logger.clone(),
            ln_dlc_wallet.clone(),
            persister.clone(),
        ));

        let keys_manager = {
            Arc::new(CustomKeysManager::new(KeysManager::new(
                &seed.lightning_seed(),
                time_since_unix_epoch.as_secs(),
                time_since_unix_epoch.subsec_nanos(),
            )))
        };

        let channel_manager = channel_manager::build(
            &ldk_data_dir,
            keys_manager.clone(),
            ln_dlc_wallet.clone(),
            tx_sync.clone(),
            logger.clone(),
            chain_monitor.clone(),
            ldk_user_config,
            network,
            persister.clone(),
        )
        .await?;
        let channel_manager = Arc::new(channel_manager);

        let sync_cman = channel_manager.clone();
        let sync_cmon = chain_monitor.clone();
        let stop_sync = Arc::clone(&stop_running);
        let sync_tx_sync = tx_sync.clone();
        tokio::spawn(async move {
            loop {
                if stop_sync.load(Ordering::Acquire) {
                    return;
                }
                let now = Instant::now();
                let confirmables = vec![
                    &*sync_cman as &(dyn Confirm + Sync + Send),
                    &*sync_cmon as &(dyn Confirm + Sync + Send),
                ];
                match sync_tx_sync.sync(confirmables).await {
                    Ok(()) => tracing::info!(
                        "Background sync of Lightning wallet finished in {}ms.",
                        now.elapsed().as_millis()
                    ),
                    Err(e) => {
                        tracing::error!("Background sync of Lightning wallet failed: {e:#}")
                    }
                }
                tokio::time::sleep(OFF_CHAIN_SYNC_INTERVAL).await;
            }
        });

        let genesis = genesis_block(network).header.block_hash();
        let network_graph_path = format!("{ldk_data_dir}/network_graph");
        let network_graph = Arc::new(disk::read_network(
            Path::new(&network_graph_path),
            genesis,
            logger.clone(),
        ));

        let gossip_sync = Arc::new(P2PGossipSync::new(
            network_graph.clone(),
            None::<Arc<dyn chain::Access + Send + Sync>>,
            logger.clone(),
        ));

        let oracle_client = oracle_client::build().await?;
        let oracle_client = Arc::new(oracle_client);

        let dlc_manager = dlc_manager::build(
            data_dir,
            ln_dlc_wallet.clone(),
            storage,
            oracle_client.clone(),
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
            keys_manager
                .get_node_secret(Recipient::Node)
                .map_err(|_| anyhow!("Could not get node's secret"))?,
            time_since_unix_epoch.as_secs() as u32,
            &ephemeral_randomness,
            logger.clone(),
            dlc_message_handler.clone(),
        ));

        let scorer_path = data_dir.join("scorer");
        let scorer = Arc::new(Mutex::new(disk::read_scorer(
            scorer_path.as_path(),
            network_graph.clone(),
            logger.clone(),
        )));

        let router = DefaultRouter::new(
            network_graph.clone(),
            logger.clone(),
            keys_manager.get_secure_random_bytes(),
            scorer.clone(),
        );

        let fake_channel_payments: FakeChannelPaymentRequests =
            Arc::new(Mutex::new(HashMap::new()));

        let payment_persister = Arc::new(payment_persister);
        let event_handler = {
            let runtime_handle = tokio::runtime::Handle::current();

            EventHandler::new(
                runtime_handle,
                channel_manager.clone(),
                ln_dlc_wallet.clone(),
                network_graph.clone(),
                keys_manager.clone(),
                payment_persister.clone(),
                fake_channel_payments.clone(),
                Arc::new(Mutex::new(HashMap::new())),
                peer_manager.clone(),
            )
        };

        let invoice_payer = Arc::new(InvoicePayer::new(
            channel_manager.clone(),
            router,
            logger.clone(),
            event_handler,
            payment::Retry::Timeout(Duration::from_secs(10)),
        ));

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

        let background_processor = BackgroundProcessor::start(
            persister.clone(),
            invoice_payer.clone(),
            chain_monitor.clone(),
            channel_manager.clone(),
            GossipSync::p2p(gossip_sync.clone()),
            peer_manager.clone(),
            logger.clone(),
            Some(scorer),
        );

        let alias = alias_as_bytes(alias)?;
        let broadcast_node_announcement_handle = {
            let announcement_addresses = announcement_addresses.clone();
            let peer_manager = peer_manager.clone();
            let (fut, remote_handle) = async move {
                let mut interval = tokio::time::interval(BROADCAST_NODE_ANNOUNCEMENT_INTERVAL);
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
                        process_pending_dlc_actions(&sub_channel_manager, &dlc_message_handler);

                        tokio::time::sleep(Duration::from_secs(10)).await;
                    }
                }
            }
            .remote_handle();

            tokio::spawn(fut);

            remote_handle
        };

        let node_info = NodeInfo {
            pubkey: channel_manager.get_our_node_id(),
            address: announcement_address,
        };

        tracing::info!("Lightning node started with node ID {}", node_info);

        Ok(Self {
            network,
            wallet: ln_dlc_wallet,
            peer_manager,
            invoice_payer,
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
            user_config: ldk_user_config,
            _background_processor: background_processor,
            _connection_manager_handle: connection_manager_handle,
            _broadcast_node_announcement_handle: broadcast_node_announcement_handle,
            _pending_dlc_actions_handle: pending_dlc_actions_handle,
            network_graph,
            #[cfg(test)]
            announcement_addresses,
            #[cfg(test)]
            alias,
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
