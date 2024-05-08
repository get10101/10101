use crate::bitcoin_conversion::to_secp_pk_30;
use crate::blockchain::Blockchain;
use crate::dlc::TracingLogger;
use crate::dlc_custom_signer::CustomKeysManager;
use crate::dlc_wallet::DlcWallet;
use crate::fee_rate_estimator::FeeRateEstimator;
use crate::message_handler::TenTenOneMessageHandler;
use crate::node::event::connect_node_event_handler_to_dlc_channel_events;
use crate::node::event::NodeEventHandler;
use crate::on_chain_wallet::BdkStorage;
use crate::on_chain_wallet::FeeConfig;
use crate::on_chain_wallet::OnChainWallet;
use crate::seed::Bip39Seed;
use crate::shadow::Shadow;
use crate::storage::DlcChannelEvent;
use crate::storage::DlcStorageProvider;
use crate::storage::TenTenOneStorage;
use crate::PeerManager;
use anyhow::Result;
use bitcoin::address::NetworkUnchecked;
use bitcoin::secp256k1::PublicKey;
use bitcoin::secp256k1::XOnlyPublicKey;
use bitcoin::Address;
use bitcoin::Network;
use bitcoin::Txid;
use futures::future::RemoteHandle;
use futures::FutureExt;
use lightning::sign::KeysManager;
use p2pd_oracle_client::P2PDOracleClient;
use serde::Deserialize;
use serde::Serialize;
use serde_with::serde_as;
use serde_with::DurationSeconds;
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;
use std::time::SystemTime;
use tokio::sync::RwLock;
use tokio::task::spawn_blocking;

mod connection;
mod dlc_manager;
mod oracle;
mod storage;
mod wallet;

pub mod dlc_channel;
pub mod event;
pub mod peer_manager;

pub use crate::message_handler::tentenone_message_name;
pub use ::dlc_manager as rust_dlc_manager;
use ::dlc_manager::ReferenceId;
pub use dlc_manager::signed_channel_state_name;
pub use dlc_manager::DlcManager;
use lightning::ln::peer_handler::ErroringMessageHandler;
use lightning::ln::peer_handler::IgnoringMessageHandler;
use lightning::ln::peer_handler::MessageHandler;
pub use oracle::OracleInfo;
use secp256k1_zkp::SECP256K1;
pub use storage::InMemoryStore;
pub use storage::Storage;
use uuid::Uuid;

/// A node.
pub struct Node<D: BdkStorage, S: TenTenOneStorage, N: Storage> {
    pub settings: Arc<RwLock<XXINodeSettings>>,
    pub network: Network,

    pub(crate) wallet: Arc<OnChainWallet<D>>,
    pub blockchain: Arc<Blockchain<N>>,

    // Making this public is only necessary because of the collaborative revert protocol.
    pub dlc_wallet: Arc<DlcWallet<D, S, N>>,

    pub peer_manager: Arc<PeerManager<D>>,
    pub keys_manager: Arc<CustomKeysManager<D>>,
    pub fee_rate_estimator: Arc<FeeRateEstimator>,

    pub logger: Arc<TracingLogger>,

    pub info: NodeInfo,

    pub dlc_manager: Arc<DlcManager<D, S, N>>,

    /// All oracles clients the node is aware of.
    pub oracles: Vec<Arc<P2PDOracleClient>>,
    pub dlc_message_handler: Arc<TenTenOneMessageHandler>,

    /// The oracle pubkey used for proposing dlc channels
    pub oracle_pubkey: XOnlyPublicKey,

    pub event_handler: Arc<NodeEventHandler>,

    // storage
    // TODO(holzeis): The node storage should get extracted to the corresponding application
    // layers.
    pub node_storage: Arc<N>,
    pub dlc_storage: Arc<DlcStorageProvider<S>>,

    // fields below are needed only to start the node
    #[allow(dead_code)]
    listen_address: SocketAddr, // Irrelevant when using websockets
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct NodeInfo {
    pub pubkey: PublicKey,
    pub address: SocketAddr,
    pub is_ws: bool,
}

/// Node is running until this struct is dropped
pub struct RunningNode {
    _handles: Vec<RemoteHandle<()>>,
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct XXINodeSettings {
    /// How often we sync the off chain wallet
    #[serde_as(as = "DurationSeconds")]
    pub off_chain_sync_interval: Duration,
    /// How often we sync the BDK wallet
    #[serde_as(as = "DurationSeconds")]
    pub on_chain_sync_interval: Duration,
    /// How often we update the fee rate
    #[serde_as(as = "DurationSeconds")]
    pub fee_rate_sync_interval: Duration,
    /// How often we run the [`SubChannelManager`]'s periodic check.
    #[serde_as(as = "DurationSeconds")]
    pub sub_channel_manager_periodic_check_interval: Duration,
    /// How often we sync the shadow states
    #[serde_as(as = "DurationSeconds")]
    pub shadow_sync_interval: Duration,
}

impl<D: BdkStorage, S: TenTenOneStorage + 'static, N: Storage + Sync + Send + 'static>
    Node<D, S, N>
{
    pub async fn update_settings(&self, new_settings: XXINodeSettings) {
        tracing::info!(?new_settings, "Updating LnDlcNode settings");
        *self.settings.write().await = new_settings;
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        alias: &str,
        network: Network,
        data_dir: &Path,
        storage: S,
        node_storage: Arc<N>,
        wallet_storage: D,
        announcement_address: SocketAddr,
        listen_address: SocketAddr,
        electrs_server_url: String,
        seed: Bip39Seed,
        ephemeral_randomness: [u8; 32],
        settings: XXINodeSettings,
        oracle_clients: Vec<P2PDOracleClient>,
        oracle_pubkey: XOnlyPublicKey,
        node_event_handler: Arc<NodeEventHandler>,
        dlc_event_sender: mpsc::Sender<DlcChannelEvent>,
    ) -> Result<Self> {
        let time_since_unix_epoch = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?;

        let logger = Arc::new(TracingLogger {
            alias: alias.to_string(),
        });

        let fee_rate_estimator = Arc::new(FeeRateEstimator::new(network));

        let on_chain_wallet = OnChainWallet::new(
            network,
            seed.wallet_seed(),
            wallet_storage,
            fee_rate_estimator.clone(),
        )?;
        let on_chain_wallet = Arc::new(on_chain_wallet);

        let blockchain = Blockchain::new(electrs_server_url.clone(), node_storage.clone())?;
        let blockchain = Arc::new(blockchain);

        let dlc_storage = Arc::new(DlcStorageProvider::new(storage.clone(), dlc_event_sender));

        let keys_manager = {
            Arc::new(CustomKeysManager::new(
                KeysManager::new(
                    &seed.lightning_seed(),
                    time_since_unix_epoch.as_secs(),
                    time_since_unix_epoch.subsec_nanos(),
                ),
                on_chain_wallet.clone(),
            ))
        };

        let oracle_clients: Vec<Arc<P2PDOracleClient>> =
            oracle_clients.into_iter().map(Arc::new).collect();

        let dlc_wallet = DlcWallet::new(
            on_chain_wallet.clone(),
            dlc_storage.clone(),
            blockchain.clone(),
        );
        let dlc_wallet = Arc::new(dlc_wallet);

        let dlc_manager = dlc_manager::build(
            data_dir,
            dlc_wallet.clone(),
            dlc_storage.clone(),
            oracle_clients.clone(),
            fee_rate_estimator.clone(),
        )?;
        let dlc_manager = Arc::new(dlc_manager);

        let dlc_message_handler =
            Arc::new(TenTenOneMessageHandler::new(node_event_handler.clone()));

        let peer_manager: Arc<PeerManager<D>> = Arc::new(PeerManager::new(
            MessageHandler {
                chan_handler: Arc::new(ErroringMessageHandler::new()),
                route_handler: Arc::new(IgnoringMessageHandler {}),
                onion_message_handler: dlc_message_handler.clone(),
                custom_message_handler: dlc_message_handler.clone(),
            },
            time_since_unix_epoch.as_secs() as u32,
            &ephemeral_randomness,
            logger.clone(),
            keys_manager.clone(),
        ));

        let node_id = keys_manager.get_node_secret_key().public_key(SECP256K1);
        let node_info = NodeInfo {
            pubkey: to_secp_pk_30(node_id),
            address: announcement_address,
            is_ws: false,
        };

        let settings = Arc::new(RwLock::new(settings));

        Ok(Self {
            network,
            wallet: on_chain_wallet,
            blockchain,
            dlc_wallet,
            peer_manager,
            keys_manager,
            logger,
            info: node_info,
            oracles: oracle_clients,
            dlc_message_handler,
            dlc_manager,
            dlc_storage,
            node_storage,
            fee_rate_estimator,
            settings,
            listen_address,
            oracle_pubkey,
            event_handler: node_event_handler,
        })
    }

    /// Starts the background handles - if the returned handles are dropped, the
    /// background tasks are stopped.
    // TODO: Consider having handles for *all* the tasks & threads for a clean shutdown.
    pub fn start(
        &self,
        dlc_event_receiver: mpsc::Receiver<DlcChannelEvent>,
    ) -> Result<RunningNode> {
        #[cfg(feature = "ln_net_tcp")]
        let handles = vec![spawn_connection_management(
            self.peer_manager.clone(),
            self.listen_address,
        )];

        #[cfg(not(feature = "ln_net_tcp"))]
        let mut handles = Vec::new();

        std::thread::spawn(shadow_sync_periodically(
            self.settings.clone(),
            self.node_storage.clone(),
            self.wallet.clone(),
        ));

        tokio::spawn(update_fee_rate_estimates(
            self.settings.clone(),
            self.fee_rate_estimator.clone(),
        ));

        connect_node_event_handler_to_dlc_channel_events(
            self.event_handler.clone(),
            dlc_event_receiver,
        );

        tracing::info!("Node started with node ID {}", self.info);

        Ok(RunningNode { _handles: handles })
    }

    /// Send the given `amount_sats` sats to the given unchecked, on-chain `address`.
    pub async fn send_to_address(
        &self,
        address: Address<NetworkUnchecked>,
        amount_sats: u64,
        fee_config: FeeConfig,
    ) -> Result<Txid> {
        let address = address.require_network(self.network)?;

        let tx = spawn_blocking({
            let wallet = self.wallet.clone();
            move || {
                let tx = wallet.build_on_chain_payment_tx(&address, amount_sats, fee_config)?;

                anyhow::Ok(tx)
            }
        })
        .await
        .expect("task to complete")?;

        let txid = self.blockchain.broadcast_transaction_blocking(&tx)?;

        Ok(txid)
    }

    pub fn list_peers(&self) -> Vec<PublicKey> {
        self.peer_manager
            .get_peer_node_ids()
            .into_iter()
            .map(|(peer, _)| to_secp_pk_30(peer))
            .collect()
    }
}

async fn update_fee_rate_estimates(
    settings: Arc<RwLock<XXINodeSettings>>,
    fee_rate_estimator: Arc<FeeRateEstimator>,
) {
    loop {
        if let Err(err) = fee_rate_estimator.update().await {
            tracing::error!("Failed to update fee rate estimates: {err:#}");
        }

        let interval = {
            let guard = settings.read().await;
            guard.fee_rate_sync_interval
        };
        tokio::time::sleep(interval).await;
    }
}

fn shadow_sync_periodically<D: BdkStorage, N: Storage>(
    settings: Arc<RwLock<XXINodeSettings>>,
    node_storage: Arc<N>,
    wallet: Arc<OnChainWallet<D>>,
) -> impl Fn() {
    let handle = tokio::runtime::Handle::current();
    let shadow = Shadow::new(node_storage, wallet);
    move || loop {
        if let Err(e) = shadow.sync_transactions() {
            tracing::error!("Failed to sync transaction shadows. Error: {e:#}");
        }

        let interval = handle.block_on(async {
            let guard = settings.read().await;
            guard.shadow_sync_interval
        });

        std::thread::sleep(interval);
    }
}

#[cfg(feature = "ln_net_tcp")]
fn spawn_connection_management<D: BdkStorage>(
    peer_manager: Arc<PeerManager<D>>,
    listen_address: SocketAddr,
) -> RemoteHandle<()> {
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
                crate::networking::tcp::setup_inbound(
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

    tracing::info!("Listening on {listen_address}");

    remote_handle
}

impl Display for NodeInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let scheme = if self.is_ws { "ws" } else { "tcp" };

        format!("{scheme}://{}@{}", self.pubkey, self.address).fmt(f)
    }
}

pub fn new_reference_id() -> ReferenceId {
    let uuid = Uuid::new_v4();
    let hex = hex::encode(uuid.as_simple().as_ref());
    let bytes = hex.as_bytes();

    debug_assert!(bytes.len() == 32, "length must be exactly 32 bytes");

    let mut array = [0u8; 32];
    array.copy_from_slice(bytes);

    array
}
