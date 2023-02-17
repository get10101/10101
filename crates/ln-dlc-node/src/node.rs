use crate::disk;
use crate::ln::event_handler::EventHandler;
use crate::ln_dlc_wallet::LnDlcWallet;
use crate::logger::TracingLogger;
use crate::on_chain_wallet::OnChainWallet;
use crate::seed::Bip39Seed;
use crate::ChainMonitor;
use crate::ChannelManager;
use crate::FakeChannelPaymentRequests;
use crate::HTLCStatus;
use crate::InvoicePayer;
use crate::NetworkGraph;
use crate::PaymentInfoStorage;
use crate::PeerManager;
use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use bdk::blockchain::ElectrumBlockchain;
use bdk::Balance;
use bitcoin::hashes::sha256;
use bitcoin::hashes::Hash;
use bitcoin::secp256k1::PublicKey;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::Network;
use dlc_manager::custom_signer::CustomKeysManager;
use dlc_messages::message_handler::MessageHandler as DlcMessageHandler;
use futures::Future;
use lightning::chain;
use lightning::chain::chainmonitor;
use lightning::chain::keysinterface::KeysInterface;
use lightning::chain::keysinterface::KeysManager;
use lightning::chain::keysinterface::Recipient;
use lightning::chain::Access;
use lightning::chain::BestBlock;
use lightning::ln::channelmanager::ChainParameters;
use lightning::ln::channelmanager::MIN_CLTV_EXPIRY_DELTA;
use lightning::ln::msgs::NetAddress;
use lightning::ln::peer_handler::IgnoringMessageHandler;
use lightning::ln::peer_handler::MessageHandler;
use lightning::routing::gossip::P2PGossipSync;
use lightning::routing::gossip::RoutingFees;
use lightning::routing::router::DefaultRouter;
use lightning::routing::router::RouteHint;
use lightning::routing::router::RouteHintHop;
use lightning::routing::scoring::ProbabilisticScorer;
use lightning::util::config::ChannelHandshakeConfig;
use lightning::util::config::ChannelHandshakeLimits;
use lightning::util::config::UserConfig;
use lightning_background_processor::BackgroundProcessor;
use lightning_background_processor::GossipSync;
use lightning_invoice::payment;
use lightning_invoice::payment::PaymentError;
use lightning_invoice::Currency;
use lightning_invoice::Invoice;
use lightning_invoice::InvoiceBuilder;
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

type TracingLoggerGossipSync =
    Arc<P2PGossipSync<Arc<NetworkGraph>, Arc<dyn Access + Send + Sync>, Arc<TracingLogger>>>;

/// An LN-DLC node.
pub struct Node {
    network: Network,

    pub wallet: Arc<LnDlcWallet>,
    alias: [u8; 32],
    peer_manager: Arc<PeerManager>,
    invoice_payer: Arc<InvoicePayer<EventHandler>>,
    channel_manager: Arc<ChannelManager>,
    chain_monitor: Arc<ChainMonitor>,
    persister: Arc<FilesystemPersister>,
    keys_manager: Arc<CustomKeysManager>,

    logger: Arc<TracingLogger>,

    pub info: NodeInfo,
    gossip_sync: TracingLoggerGossipSync,
    scorer: Arc<Mutex<ProbabilisticScorer<Arc<NetworkGraph>, Arc<TracingLogger>>>>,
    fake_channel_payments: FakeChannelPaymentRequests,
}

#[derive(Debug, Clone, Copy)]
pub struct NodeInfo {
    pub pubkey: PublicKey,
    pub address: SocketAddr,
}

#[derive(Debug, Clone)]
pub struct OffChain {
    pub available: u64,
    pub pending_close: u64,
}

impl Display for NodeInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        format!("{}@{}", self.pubkey, self.address).fmt(f)
    }
}

/// Liquidity-based routing fee in millionths of a routed amount. In other words, 10000 is 1%.
const MILLIONTH_ROUTING_FEE: u32 = 20000;

impl Node {
    /// Constructs a new node to be run as the app
    pub async fn new_app(
        alias: String,
        network: Network,
        data_dir: &Path,
        address: SocketAddr,
        electrs_origin: String,
        seed: Bip39Seed,
        ephemeral_randomness: [u8; 32],
    ) -> Self {
        let user_config = app_config();
        Node::new(
            alias,
            network,
            data_dir,
            address,
            electrs_origin,
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
    pub async fn new_coordinator(
        alias: String,
        network: Network,
        data_dir: &Path,
        address: SocketAddr,
        electrs_origin: String,
        seed: Bip39Seed,
        ephemeral_randomness: [u8; 32],
    ) -> Self {
        let user_config = coordinator_config();
        Self::new(
            alias,
            network,
            data_dir,
            address,
            electrs_origin,
            seed,
            ephemeral_randomness,
            user_config,
        )
        .await
    }

    // I'd like this to be a pure function and to be able to pass in anything that was loaded from
    // the persistence layer. But we're not there yet because we're still copying convenient code
    // from `ldk-sample` which involves IO.
    #[allow(clippy::too_many_arguments)]
    async fn new(
        alias: String,
        network: Network,
        data_dir: &Path,
        address: SocketAddr,
        electrs_origin: String,
        seed: Bip39Seed,
        ephemeral_randomness: [u8; 32],
        ldk_user_config: UserConfig,
    ) -> Self {
        let time_since_unix_epoch = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap();

        let logger = Arc::new(TracingLogger);

        if !data_dir.exists() {
            std::fs::create_dir_all(data_dir)
                .context(format!("Could not create data dir ({data_dir:?})"))
                .unwrap();
        }

        let persister_path = data_dir.as_os_str().to_str().unwrap();
        let persister = Arc::new(FilesystemPersister::new(persister_path.to_string()));

        let on_chain_dir = data_dir.join("on_chain");
        let on_chain_wallet =
            OnChainWallet::new(on_chain_dir.as_path(), network, seed.wallet_seed()).unwrap();

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
            Some(ln_dlc_wallet.clone()),
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
                keys_manager.clone(),
                inbound_payments,
                outbound_payments,
                fake_channel_payments.clone(),
                Arc::new(Mutex::new(HashMap::new())),
            )
        };

        let invoice_payer = Arc::new(InvoicePayer::new(
            channel_manager.clone(),
            router,
            logger.clone(),
            event_handler,
            payment::Retry::Timeout(Duration::from_secs(10)),
        ));

        let alias = {
            if alias.len() > 32 {
                panic!("Node Alias can not be longer than 32 bytes");
            }
            let mut bytes = [0; 32];
            bytes[..alias.len()].copy_from_slice(alias.as_bytes());
            bytes
        };

        Self {
            network,
            wallet: ln_dlc_wallet,
            alias,
            peer_manager,
            persister,
            invoice_payer,
            gossip_sync,
            scorer,
            keys_manager,
            chain_monitor,
            logger,
            channel_manager: channel_manager.clone(),
            info: NodeInfo {
                pubkey: channel_manager.get_our_node_id(),
                address,
            },
            fake_channel_payments,
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

        let background_processor = BackgroundProcessor::start(
            self.persister.clone(),
            self.invoice_payer.clone(),
            self.chain_monitor.clone(),
            self.channel_manager.clone(),
            GossipSync::p2p(self.gossip_sync.clone()),
            self.peer_manager.clone(),
            self.logger.clone(),
            Some(self.scorer.clone()),
        );

        let peer_man = Arc::clone(&self.peer_manager);

        let alias = self.alias;
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            loop {
                peer_man.broadcast_node_announcement(
                    [0; 3],
                    alias,
                    vec![NetAddress::IPv4 {
                        addr: [127, 0, 0, 1],
                        port: address.port(),
                    }],
                );
                interval.tick().await;
            }
        });

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
        let temp_channel_id = self
            .channel_manager
            .create_channel(
                peer.pubkey,
                channel_amount_sat,
                initial_send_amount_sats * 1000,
                0,
                None,
            )
            .map_err(|e| anyhow!("Could not create channel with {} due to {e:?}", peer))?;

        let temp_channel_id = hex::encode(temp_channel_id);
        tracing::info!(%peer, %temp_channel_id, "Started channel creation");

        Ok(())
    }

    #[allow(dead_code)] // Clippy is in the wrong here, this code is being used in the system tests
    #[cfg(feature = "system_tests")]
    pub(crate) fn channel_manager(&self) -> &ChannelManager {
        &self.channel_manager
    }

    pub fn sync(&self) {
        let confirmables = vec![
            &*self.channel_manager as &dyn chain::Confirm,
            &*self.chain_monitor as &dyn chain::Confirm,
        ];

        self.wallet.inner().sync(confirmables).unwrap();
    }

    pub fn create_invoice(&self, amount_in_sats: u64) -> Result<Invoice> {
        let currency = match self.network {
            Network::Bitcoin => Currency::Bitcoin,
            Network::Testnet => Currency::BitcoinTestnet,
            Network::Regtest => Currency::Regtest,
            Network::Signet => Currency::Signet,
        };

        lightning_invoice::utils::create_invoice_from_channelmanager(
            &self.channel_manager,
            self.keys_manager.clone(),
            self.logger.clone(),
            currency,
            Some(amount_in_sats * 1000),
            "".to_string(),
            180,
        )
        .map_err(|e| anyhow!(e))
    }

    ///  Creates an invoice which is meant to be intercepted
    ///
    /// Doing so we need to pass in `intercepted_channel_id` which needs to be generated by the
    /// intercepting node. This information, in combination with `hop_before_me` is used to add a
    /// routing hint to the invoice. Otherwise the sending node does not know how to pay the
    /// invoice
    pub fn create_interceptable_invoice(
        &self,
        amount_in_sats: u64,
        intercepted_channel_id: u64,
        hop_before_me: PublicKey,
        invoice_expiry: u32,
        description: String,
    ) -> Result<Invoice> {
        let (payment_hash, payment_secret) = self
            .channel_manager
            .create_inbound_payment(Some(amount_in_sats * 1000), invoice_expiry)
            .unwrap();
        let node_secret = self.keys_manager.get_node_secret(Recipient::Node).unwrap();
        let signed_invoice = InvoiceBuilder::new(Currency::Regtest)
            .description(description)
            .amount_milli_satoshis(amount_in_sats * 1000)
            .payment_hash(sha256::Hash::from_slice(&payment_hash.0)?)
            .payment_secret(payment_secret)
            .timestamp(SystemTime::now())
            .private_route(RouteHint(vec![RouteHintHop {
                src_node_id: hop_before_me,
                short_channel_id: intercepted_channel_id,
                fees: RoutingFees {
                    base_msat: 1000,
                    proportional_millionths: MILLIONTH_ROUTING_FEE,
                },
                cltv_expiry_delta: MIN_CLTV_EXPIRY_DELTA,
                htlc_minimum_msat: None,
                htlc_maximum_msat: None,
            }]))
            .build_raw()
            .unwrap()
            .sign::<_, ()>(|hash| {
                let secp_ctx = Secp256k1::new();
                Ok(secp_ctx.sign_ecdsa_recoverable(hash, &node_secret))
            })
            .unwrap();
        let invoice = Invoice::from_signed(signed_invoice).unwrap();
        Ok(invoice)
    }

    pub fn send_payment(&self, invoice: &Invoice) -> Result<()> {
        match self.invoice_payer.pay_invoice(invoice) {
            Ok(_payment_id) => {
                let payee_pubkey = invoice.recover_payee_pub_key();
                let amt_msat = invoice
                    .amount_milli_satoshis()
                    .context("invalid msat amount in the invoice")?;
                tracing::info!("EVENT: initiated sending {amt_msat} msats to {payee_pubkey}",);
                HTLCStatus::Pending
            }
            Err(PaymentError::Invoice(err)) => {
                tracing::error!(%err, "Invalid invoice");
                anyhow::bail!(err);
            }
            Err(PaymentError::Routing(err)) => {
                tracing::error!(?err, "Failed to find route");
                anyhow::bail!("{:?}", err);
            }
            Err(PaymentError::Sending(err)) => {
                tracing::error!(?err, "Failed to send payment");
                HTLCStatus::Failed
            }
        };
        Ok(())
    }

    pub fn get_on_chain_balance(&self) -> Result<Balance> {
        self.wallet.inner().get_balance().map_err(|e| anyhow!(e))
    }

    /// The LDK [`OffChain`] balance keeps track of:
    ///
    /// - The total sum of money in all open channels.
    /// - The total sum of money in close transactions that do not yet pay to our on-chain wallet.
    pub fn get_ldk_balance(&self) -> OffChain {
        let open_channels = self.channel_manager.list_channels();

        let claimable_channel_balances = {
            let ignored_channels = open_channels.iter().collect::<Vec<_>>();
            let ignored_channels = &ignored_channels.as_slice();
            self.chain_monitor.get_claimable_balances(ignored_channels)
        };

        let pending_close = claimable_channel_balances.iter().fold(0, |acc, balance| {
            tracing::trace!("Pending on-chain balance from channel closure: {balance:?}");

            use ::lightning::chain::channelmonitor::Balance::*;
            match balance {
                ClaimableOnChannelClose {
                    claimable_amount_satoshis,
                }
                | ClaimableAwaitingConfirmations {
                    claimable_amount_satoshis,
                    ..
                }
                | ContentiousClaimable {
                    claimable_amount_satoshis,
                    ..
                }
                | MaybeTimeoutClaimableHTLC {
                    claimable_amount_satoshis,
                    ..
                }
                | MaybePreimageClaimableHTLC {
                    claimable_amount_satoshis,
                    ..
                }
                | CounterpartyRevokedOutputClaimable {
                    claimable_amount_satoshis,
                } => acc + claimable_amount_satoshis,
            }
        });

        let available = self
            .channel_manager
            .list_channels()
            .iter()
            .map(|details| details.balance_msat / 1000)
            .sum();

        OffChain {
            available,
            pending_close,
        }
    }

    /// Creates a fake channel id needed to intercept payments to the provided `target_node`
    ///
    /// This is mainly used for instant payments where the receiver does not have a lightning
    /// channel yet, e.g. Alice does not have a channel with Bob yet but wants to
    /// receive a LN payment. Clair pays to Bob who opens a channel to Alice and pays her.
    pub fn create_intercept_scid(&self, target_node: PublicKey) -> u64 {
        let intercept_scid = self.channel_manager.get_intercept_scid();
        self.fake_channel_payments
            .lock()
            .unwrap()
            .insert(intercept_scid, target_node);
        intercept_scid
    }
}

fn app_config() -> UserConfig {
    UserConfig {
        channel_handshake_config: ChannelHandshakeConfig {
            // this is needed as otherwise the config between the coordinator and us diverges and we
            // can't open channels.
            announced_channel: true,
            minimum_depth: 1,
            // only 10% of the total channel value can be sent. e.g. with a volume of 30.000 sats
            // only 3.000 sats can be sent.
            max_inbound_htlc_value_in_flight_percent_of_channel: 10,
            ..Default::default()
        },
        channel_handshake_limits: ChannelHandshakeLimits {
            max_minimum_depth: 1,
            // lnd's max to_self_delay is 2016, so we want to be compatible.
            their_to_self_delay: 2016,
            ..Default::default()
        },
        // we want to accept 0-conf channels from the coordinator
        manually_accept_inbound_channels: true,
        ..Default::default()
    }
}

fn coordinator_config() -> UserConfig {
    UserConfig {
        channel_handshake_config: ChannelHandshakeConfig {
            announced_channel: true,
            minimum_depth: 1,
            // only 10% of the total channel value can be sent. e.g. with a volume of 30.000 sats
            // only 3.000 sats can be sent.
            max_inbound_htlc_value_in_flight_percent_of_channel: 10,
            ..Default::default()
        },
        channel_handshake_limits: ChannelHandshakeLimits {
            max_minimum_depth: 1,
            trust_own_funding_0conf: true,
            force_announced_channel_preference: false,
            // lnd's max to_self_delay is 2016, so we want to be compatible.
            their_to_self_delay: 2016,
            ..Default::default()
        },
        accept_intercept_htlcs: true,
        accept_forwards_to_priv_channels: true,
        manually_accept_inbound_channels: true,
        ..Default::default()
    }
}
