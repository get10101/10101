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
use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use bdk::blockchain::ElectrumBlockchain;
use bitcoin::blockdata::constants::genesis_block;
use bitcoin::secp256k1::PublicKey;
use dlc_manager::custom_signer::CustomKeysManager;
use dlc_messages::message_handler::MessageHandler as DlcMessageHandler;
use futures::Future;
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
use lightning::util::config::ChannelHandshakeConfig;
use lightning::util::config::ChannelHandshakeLimits;
use lightning::util::config::UserConfig;
use lightning_background_processor::BackgroundProcessor;
use lightning_background_processor::GossipSync;
use lightning_invoice::payment;
use lightning_persister::FilesystemPersister;
use std::collections::HashMap;
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;
use std::net::SocketAddr;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::time::SystemTime;
use bitcoin::{Block, BlockHash, Txid};
use futures::stream::iter;
use tokio::task::JoinHandle;

/// An LN-DLC node.
pub struct Node {
    network: bitcoin::Network,

    pub wallet: Arc<LnDlcWallet>,
    peer_manager: Arc<PeerManager>,
    invoice_payer: Arc<InvoicePayer<EventHandler>>,
    channel_manager: Arc<ChannelManager>,
    chain_monitor: Arc<ChainMonitor>,
    persister: Arc<FilesystemPersister>,

    logger: Arc<TracingLogger>,

    data_dir: String,

    pub info: NodeInfo,
}

#[derive(Debug, Clone, Copy)]
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
        electrs_origin: String,
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

        let (height, header) = ln_dlc_wallet.tip().unwrap();
        let hash = header.block_hash();

        let channel_manager = {
            let chain_params = ChainParameters {
                network,
                // TODO: This needs to be fetched from electrs if the node is restarted. Also, I'm
                // not sure if the genesis block with a block height of 0 is a valid `BestBlock`
                best_block: BestBlock::new(hash, height),
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
        let network_graph = Arc::new(NetworkGraph::new(hash, logger.clone()));

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
            logger.clone(),
            event_handler,
            payment::Retry::Timeout(Duration::from_secs(10)),
        ));

        Self {
            network,
            wallet: ln_dlc_wallet,
            peer_manager,
            data_dir,
            persister,
            invoice_payer,
            chain_monitor,
            logger,
            channel_manager: channel_manager.clone(),
            info: NodeInfo {
                pubkey: channel_manager.get_our_node_id(),
                address,
            },
        }
    }

    pub async fn start(&self) -> Result<BackgroundProcessor> {
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

        tracing::info!("Starting background processor");

        let genesis = genesis_block(self.network).header.block_hash();
        let network_graph_path = format!("{}/network_graph", self.data_dir);
        let network_graph = Arc::new(disk::read_network(
            Path::new(&network_graph_path),
            genesis,
            self.logger.clone(),
        ));

        let gossip_sync = Arc::new(P2PGossipSync::new(
            Arc::clone(&network_graph),
            None::<Arc<dyn chain::Access + Send + Sync>>,
            self.logger.clone(),
        ));

        // Step 17: Initialize routing ProbabilisticScorer
        let scorer_path = format!("{}/scorer", self.data_dir);
        let scorer = Arc::new(Mutex::new(disk::read_scorer(
            Path::new(&scorer_path),
            Arc::clone(&network_graph),
            Arc::clone(&self.logger),
        )));

        let background_processor = BackgroundProcessor::start(
            self.persister.clone(),
            self.invoice_payer.clone(),
            self.chain_monitor.clone(),
            self.channel_manager.clone(),
            GossipSync::p2p(gossip_sync.clone()),
            self.peer_manager.clone(),
            self.logger.clone(),
            Some(scorer),
        );

        tracing::info!(
            "Lightning node started with node ID {}@{}",
            self.info.pubkey,
            self.info.address
        );
        Ok(background_processor)
    }

    async fn connect(
        peer_manager: Arc<PeerManager>,
        peer: NodeInfo,
    ) -> Result<Pin<Box<impl Future<Output = ()>>>> {
        let connection_closed_future =
            lightning_net_tokio::connect_outbound(peer_manager.clone(), peer.pubkey, peer.address)
                .await
                .context("Failed to connect to counterparty")?;

        let mut connection_closed_future = Box::pin(connection_closed_future);
        while !Self::is_connected(&peer_manager, peer.pubkey) {
            if futures::poll!(&mut connection_closed_future).is_ready() {
                bail!("Peer disconnected before we finished the handshake");
            }

            tracing::debug!(%peer, "Waiting to establish connection");
            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        tracing::info!(%peer, "Connection established");
        Ok(connection_closed_future)
    }

    // todo: That might be better placed in a dedicated connection manager file.
    pub async fn keep_connected(&self, peer: NodeInfo) -> Result<()> {
        let connection_closed_future = Self::connect(self.peer_manager.clone(), peer).await?;

        let peer_manager = self.peer_manager.clone();
        tokio::spawn({
            async move {
                let mut connection_closed_future = connection_closed_future;

                loop {
                    tracing::debug!(%peer, "Keeping connection alive");

                    connection_closed_future.await;
                    tracing::debug!(%peer, "Connection lost");

                    loop {
                        match Self::connect(peer_manager.clone(), peer).await {
                            Ok(fut) => {
                                connection_closed_future = fut;
                                break;
                            }
                            Err(_) => continue,
                        }
                    }
                }
            }
        });

        Ok(())
    }

    fn is_connected(peer_manager: &Arc<PeerManager>, pubkey: PublicKey) -> bool {
        peer_manager
            .get_peer_node_ids()
            .iter()
            .any(|id| *id == pubkey)
    }

    pub fn open_channel(
        &self,
        peer: NodeInfo,
        channel_amount_sat: u64,
        initial_send_amount_sats: u64,
    ) -> Result<()> {
        let user_config = default_user_config();
        let temp_channel_id = self
            .channel_manager
            .create_channel(
                peer.pubkey,
                channel_amount_sat,
                initial_send_amount_sats * 1000,
                0,
                Some(user_config),
            )
            .map_err(|e| anyhow!("Could not create channel with {} due to {e:?}", peer))?;

        let temp_channel_id = hex::encode(temp_channel_id);
        tracing::info!(%peer, %temp_channel_id, "Started channel creation");

        Ok(())
    }

    pub(crate) fn channel_manager(&self) -> &ChannelManager {
        &self.channel_manager
    }
}

fn default_user_config() -> UserConfig {
    UserConfig {
        channel_handshake_limits: ChannelHandshakeLimits {
            trust_own_funding_0conf: false,
            ..Default::default()
        },
        channel_handshake_config: ChannelHandshakeConfig {
            announced_channel: true,
            minimum_depth: 1,
            ..Default::default()
        },
        // By setting `manually_accept_inbound_channels` to `true` we need to manually confirm every
        // inbound channel request.
        manually_accept_inbound_channels: false,
        ..Default::default()
    }
}
