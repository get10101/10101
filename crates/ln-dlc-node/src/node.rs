use crate::disk;
use crate::ln::event_handler::EventHandler;
use crate::ln_dlc_wallet::LnDlcWallet;
use crate::on_chain_wallet::OnChainWallet;
use crate::ChainMonitor;
use crate::ChannelManager;
use crate::InvoicePayer;
use crate::NetworkGraph;
use crate::PaymentInfoStorage;
use crate::PeerManager;
use crate::TracingLogger;
use anyhow::Result;
use bdk::blockchain::ElectrumBlockchain;
use bitcoin::blockdata::constants::genesis_block;
use bitcoin::secp256k1::PublicKey;
use dlc_manager::custom_signer::CustomKeysManager;
use dlc_messages::message_handler::MessageHandler as DlcMessageHandler;
use lightning::chain;
use lightning::chain::chainmonitor;
use lightning::chain::keysinterface::KeysInterface;
use lightning::chain::keysinterface::KeysManager;
use lightning::chain::BestBlock;
use lightning::ln::channelmanager::ChainParameters;
use lightning::ln::peer_handler::IgnoringMessageHandler;
use lightning::ln::peer_handler::MessageHandler;
use lightning::routing::gossip::P2PGossipSync;
use lightning::routing::router::DefaultRouter;
use lightning::util::config::ChannelHandshakeLimits;
use lightning_invoice::payment;
use lightning_persister::FilesystemPersister;
use std::collections::HashMap;
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::time::SystemTime;

/// An LN-DLC node.
pub struct Node {
    network: bitcoin::Network,

    wallet: Arc<LnDlcWallet>,
    peer_manager: Arc<PeerManager>,
    invoice_payer: Arc<InvoicePayer<EventHandler>>,

    data_dir: String,

    info: NodeInfo,
}

#[derive(Clone, Copy)]
pub struct NodeInfo {
    pub pubkey: PublicKey,
    pub address: SocketAddr,
}

impl Display for NodeInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        format!("{}@{}", self.pubkey, self.address).fmt(f)
    }
}

impl Node {
    // I'd like this to be a pure function and to be able to pass in anything that was loaded from
    // the persistence layer. But we're not there yet because we're still copying convenient code
    // from `ldk-sample` which involves IO.
    pub async fn new(
        network: bitcoin::Network,
        data_dir: String,
        address: SocketAddr,
        electrs_origin: String, // "http://localhost:30000/".to_string()
        seed: [u8; 32],
        ephemeral_randomness: [u8; 32],
    ) -> Self {
        let time_since_unix_epoch = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap();

        let logger = Arc::new(TracingLogger);

        // TODO: Might be better to use an in-memory persister for the tests.
        let persister = Arc::new(FilesystemPersister::new(data_dir.clone()));

        let on_chain_wallet =
            OnChainWallet::new(Path::new(&format!("{}/on_chain", data_dir)), network).unwrap();

        let ln_dlc_wallet = {
            let blockchain_client = ElectrumBlockchain::from(
                bdk::electrum_client::Client::new(&electrs_origin).unwrap(),
            );
            Arc::new(LnDlcWallet::new(
                Box::new(blockchain_client),
                on_chain_wallet.inner,
            ))
        };

        let chain_monitor: Arc<ChainMonitor> = Arc::new(chainmonitor::ChainMonitor::new(
            None,
            ln_dlc_wallet.clone(),
            logger.clone(),
            ln_dlc_wallet.clone(),
            persister,
        ));

        let keys_manager = {
            Arc::new(CustomKeysManager::new(KeysManager::new(
                &seed,
                time_since_unix_epoch.as_secs() as u64,
                time_since_unix_epoch.subsec_nanos() as u32,
            )))
        };

        let ldk_user_config = lightning::util::config::UserConfig {
            channel_handshake_config: lightning::util::config::ChannelHandshakeConfig {
                max_inbound_htlc_value_in_flight_percent_of_channel: 50,
                ..Default::default()
            },
            channel_handshake_limits: ChannelHandshakeLimits {
                force_announced_channel_preference: false,
                ..Default::default()
            },
            ..Default::default()
        };

        let genesis_block_hash = genesis_block(network).header.block_hash();

        let channel_manager = {
            let chain_params = ChainParameters {
                network,
                // TODO: This needs to be fetched from electrs if the node is restarted. Also, I'm
                // not sure if the genesis block with a block height of 0 is a valid `BestBlock`
                best_block: BestBlock::new(genesis_block_hash, 0),
            };
            Arc::new(ChannelManager::new(
                ln_dlc_wallet.clone(),
                chain_monitor.clone(),
                ln_dlc_wallet.clone(),
                logger.clone(),
                keys_manager.clone(),
                ldk_user_config,
                chain_params,
            ))
        };

        // TODO: Provide persisted one if restarting
        let network_graph = Arc::new(NetworkGraph::new(genesis_block_hash, logger.clone()));

        let gossip_sync = Arc::new(P2PGossipSync::new(
            network_graph.clone(),
            None::<Arc<dyn chain::Access + Send + Sync>>,
            logger.clone(),
        ));

        let lightning_msg_handler = MessageHandler {
            chan_handler: channel_manager.clone(),
            route_handler: gossip_sync.clone(),
            onion_message_handler: Arc::new(IgnoringMessageHandler {}),
        };

        let dlc_message_handler = Arc::new(DlcMessageHandler::new());

        let peer_manager: Arc<PeerManager> = Arc::new(PeerManager::new(
            lightning_msg_handler,
            keys_manager
                .get_node_secret(chain::keysinterface::Recipient::Node)
                .unwrap(),
            time_since_unix_epoch.as_secs() as u32,
            &ephemeral_randomness,
            logger.clone(),
            dlc_message_handler,
        ));

        let scorer = Arc::new(Mutex::new(disk::read_scorer(
            Path::new(&format!("{}/scorer", data_dir)),
            network_graph.clone(),
            logger.clone(),
        )));

        let router = DefaultRouter::new(
            network_graph.clone(),
            logger.clone(),
            keys_manager.get_secure_random_bytes(),
            scorer,
        );

        let event_handler = {
            let runtime_handle = tokio::runtime::Handle::current();

            // TODO: Persist payment info to disk
            let inbound_payments: PaymentInfoStorage = Arc::new(Mutex::new(HashMap::new()));
            let outbound_payments: PaymentInfoStorage = Arc::new(Mutex::new(HashMap::new()));

            EventHandler::new(
                runtime_handle,
                channel_manager.clone(),
                ln_dlc_wallet.clone(),
                network_graph,
                keys_manager,
                inbound_payments,
                outbound_payments,
            )
        };

        let invoice_payer = Arc::new(InvoicePayer::new(
            channel_manager.clone(),
            router,
            logger,
            event_handler,
            payment::Retry::Timeout(Duration::from_secs(10)),
        ));

        Self {
            network,
            wallet: ln_dlc_wallet,
            peer_manager,
            data_dir,
            invoice_payer,
            info: NodeInfo {
                pubkey: channel_manager.get_our_node_id(),
                address,
            },
        }
    }

    pub async fn start(&self) -> Result<()> {
        let address = self.info.address;

        // Connection manager
        tokio::spawn({
            let peer_manager = self.peer_manager.clone();
            async move {
                let listener = tokio::net::TcpListener::bind(address)
                    .await
                    .expect("Failed to bind to listen port");
                loop {
                    let peer_manager = peer_manager.clone();
                    let (tcp_stream, _) = listener.accept().await.unwrap();

                    tokio::spawn(async move {
                        lightning_net_tokio::setup_inbound(
                            peer_manager.clone(),
                            tcp_stream.into_std().unwrap(),
                        )
                        .await;
                    });
                }
            }
        });
        // TODO: Call sync(?) in a loop

        tracing::info!("Listening on {address}");

        Ok(())
    }

    pub async fn connect(&self, target: Node) -> Result<()> {
        match lightning_net_tokio::connect_outbound(
            self.peer_manager.clone(),
            target.info.pubkey,
            target.info.address,
        )
        .await
        {
            Some(connection_closed_future) => {
                let mut connection_closed_future = Box::pin(connection_closed_future);
                while !Self::is_connected(&self.peer_manager, target.info.pubkey) {
                    if futures::poll!(&mut connection_closed_future).is_ready() {
                        tracing::warn!("Peer disconnected before we finished the handshake! Retrying in 5 seconds.");
                        tokio::time::sleep(Duration::from_secs(5)).await;
                        continue;
                    }
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
                tracing::info!("Successfully connected to {}", target.info);
                connection_closed_future.await;
                tracing::warn!("Lost connection to maker, retrying immediately.")
            }
            None => {
                tracing::warn!("Failed to connect to maker! Retrying.");
            }
        }
        Ok(())
    }

    fn is_connected(peer_manager: &Arc<PeerManager>, pubkey: PublicKey) -> bool {
        peer_manager
            .get_peer_node_ids()
            .iter()
            .any(|id| *id == pubkey)
    }
}
