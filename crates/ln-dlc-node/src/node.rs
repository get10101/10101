use crate::ChainMonitor;
use crate::ChannelManager;
use crate::NetworkGraph;
use crate::PeerManager;
use crate::TracingLogger;
use anyhow::Result;
use bitcoin::blockdata::constants::genesis_block;
use dlc_manager::custom_signer::CustomKeysManager;
use dlc_messages::message_handler::MessageHandler as DlcMessageHandler;
use electrs_blockchain_provider::ElectrsBlockchainProvider;
use lightning::chain;
use lightning::chain::chainmonitor;
use lightning::chain::keysinterface::KeysInterface;
use lightning::chain::keysinterface::KeysManager;
use lightning::chain::BestBlock;
use lightning::ln::channelmanager::ChainParameters;
use lightning::ln::peer_handler::IgnoringMessageHandler;
use lightning::ln::peer_handler::MessageHandler;
use lightning::routing::gossip::P2PGossipSync;
use lightning::util::config::ChannelHandshakeLimits;
use lightning_persister::FilesystemPersister;
use std::sync::Arc;
use std::time::SystemTime;

/// An LN-DLC node.
pub struct Node {
    network: bitcoin::Network,

    electrs_client: Arc<ElectrsBlockchainProvider>,
    peer_manager: Arc<PeerManager>,

    ln_listening_port: u16,

    data_dir: String,
}

impl Node {
    // TODO: I'd like this to be synchronous
    pub async fn new(
        network: bitcoin::Network,
        data_dir: String,
        ln_listening_port: u16,
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

        // TODO: This spawns a long-running task which to me is unexpected in a constructor
        let electrs_client = tokio::task::spawn_blocking(move || {
            Arc::new(ElectrsBlockchainProvider::new(electrs_origin, network))
        })
        .await
        .unwrap();

        let chain_monitor: Arc<ChainMonitor> = Arc::new(chainmonitor::ChainMonitor::new(
            None,
            electrs_client.clone(),
            logger.clone(),
            electrs_client.clone(),
            persister.clone(),
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
                electrs_client.clone(),
                chain_monitor.clone(),
                electrs_client.clone(),
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

        Self {
            network,
            electrs_client,
            peer_manager,
            ln_listening_port,
            data_dir,
        }
    }

    pub async fn start(&self) -> Result<()> {
        // Connection manager
        tokio::spawn({
            let peer_manager = self.peer_manager.clone();
            let ln_listening_port = self.ln_listening_port;
            async move {
                let listener =
                    tokio::net::TcpListener::bind(format!("0.0.0.0:{}", ln_listening_port))
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

        // TODO: Connect and disconnect blocks

        Ok(())
    }
}
