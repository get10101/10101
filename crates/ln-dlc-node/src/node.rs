use crate::disk;
use crate::ln::event_handler::EventHandler;
use crate::ln_dlc_wallet::LnDlcWallet;
use crate::logger::TracingLogger;
use crate::on_chain_wallet::OnChainWallet;
use crate::seed::Bip39Seed;
use crate::ChainMonitor;
use crate::ChannelManager;
use crate::DlcManager;
use crate::FakeChannelPaymentRequests;
use crate::HTLCStatus;
use crate::InvoicePayer;
use crate::NetworkGraph;
use crate::PaymentInfoStorage;
use crate::PeerManager;
use crate::SubChannelManager;
use anyhow::anyhow;
use anyhow::Context;
use anyhow::Error;
use anyhow::Result;
use bdk::blockchain::ElectrumBlockchain;
use bdk::Balance;
use bitcoin::hashes::sha256;
use bitcoin::hashes::Hash;
use bitcoin::secp256k1::PublicKey;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::Network;
use dlc_manager::contract::contract_input::ContractInput as DlcContractInput;
use dlc_manager::contract::Contract;
use dlc_manager::custom_signer::CustomKeysManager;
use dlc_manager::sub_channel_manager::SubChannelState;
use dlc_manager::Oracle;
use dlc_manager::Storage;
use dlc_manager::SystemTimeProvider;
use dlc_messages::message_handler::MessageHandler as DlcMessageHandler;
use dlc_messages::sub_channel::SubChannelMessage;
use dlc_sled_storage_provider::SledStorageProvider;
use lightning::chain;
use lightning::chain::chainmonitor;
use lightning::chain::keysinterface::KeysInterface;
use lightning::chain::keysinterface::KeysManager;
use lightning::chain::keysinterface::Recipient;
use lightning::chain::Access;
use lightning::chain::BestBlock;
use lightning::ln::channelmanager::ChainParameters;
use lightning::ln::channelmanager::ChannelDetails;
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
use lightning::util::config::ChannelConfig;
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
use p2pd_oracle_client::P2PDOracleClient;
use serde::Deserialize;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;
use std::fs;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::time::SystemTime;

mod connection;

type TracingLoggerGossipSync =
    Arc<P2PGossipSync<Arc<NetworkGraph>, Arc<dyn Access + Send + Sync>, Arc<TracingLogger>>>;

/// An LN-DLC node.
pub struct Node {
    network: Network,

    pub wallet: Arc<LnDlcWallet>,
    alias: [u8; 32],
    pub(crate) peer_manager: Arc<PeerManager>,
    invoice_payer: Arc<InvoicePayer<EventHandler>>,
    pub(crate) channel_manager: Arc<ChannelManager>,
    chain_monitor: Arc<ChainMonitor>,
    persister: Arc<FilesystemPersister>,
    keys_manager: Arc<CustomKeysManager>,

    logger: Arc<TracingLogger>,

    pub info: NodeInfo,
    gossip_sync: TracingLoggerGossipSync,
    scorer: Arc<Mutex<ProbabilisticScorer<Arc<NetworkGraph>, Arc<TracingLogger>>>>,
    fake_channel_payments: FakeChannelPaymentRequests,

    pub(crate) dlc_manager: Arc<DlcManager>,
    sub_channel_manager: Arc<SubChannelManager>,
    pub(crate) oracle: Arc<P2PDOracleClient>,
    dlc_message_handler: Arc<DlcMessageHandler>,

    #[cfg(test)]
    pub(crate) user_config: UserConfig,

    pending_trades: Arc<Mutex<HashSet<PublicKey>>>,
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

/// Liquidity-based routing fee in millionths of a routed amount. In
/// other words, 10000 is 1%.
pub(crate) const LIQUIDITY_ROUTING_FEE_MILLIONTHS: u32 = 20_000;

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
    pub(crate) async fn new(
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

        let storage = Arc::new(
            SledStorageProvider::new(data_dir.to_str().expect("data_dir"))
                .expect("to be able to create sled storage"),
        );

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
                storage.clone(),
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
            keys_manager.get_node_secret(Recipient::Node).unwrap(),
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

        let offers_path = data_dir.join("offers");
        fs::create_dir_all(offers_path).expect("Error creating offered contract directory");

        let p2pdoracle = tokio::task::spawn_blocking(move || {
            Arc::new(
                P2PDOracleClient::new("https://oracle.holzeis.me/")
                    .expect("to be able to create the p2pd oracle"),
            )
        })
        .await
        .unwrap();

        let oracle_pubkey = p2pdoracle.get_public_key();
        let oracles = HashMap::from([(oracle_pubkey, p2pdoracle.clone())]);

        let dlc_manager = Arc::new(
            DlcManager::new(
                ln_dlc_wallet.clone(),
                ln_dlc_wallet.clone(),
                storage.clone(),
                oracles,
                Arc::new(SystemTimeProvider {}),
                ln_dlc_wallet.clone(),
            )
            .unwrap(),
        );

        let sub_channel_manager = Arc::new(SubChannelManager::new(
            Secp256k1::new(),
            ln_dlc_wallet.clone(),
            channel_manager.clone(),
            storage,
            ln_dlc_wallet.clone(),
            dlc_manager.clone(),
            ln_dlc_wallet.clone(),
            height as u64,
        ));

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
            sub_channel_manager,
            oracle: p2pdoracle,
            dlc_message_handler,
            dlc_manager,
            #[cfg(test)]
            user_config: ldk_user_config,
            pending_trades: Arc::new(Mutex::new(HashSet::new())),
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
                    let (tcp_stream, addr) = listener.accept().await.unwrap();

                    tracing::debug!(%addr, "Received inbound connection");

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

        let dlc_man = self.dlc_manager.clone();
        let pending_trades = self.pending_trades.clone();
        let sub_channel_manager = self.sub_channel_manager.clone();
        let dlc_message_handler = self.dlc_message_handler.clone();
        let peer_manager = self.peer_manager.clone();

        tokio::spawn({
            let dlc_man = dlc_man.clone();
            let sub_channel_manager = sub_channel_manager.clone();
            let dlc_message_handler = dlc_message_handler.clone();
            let peer_manager = peer_manager.clone();

            async move {
                loop {
                    Node::process_incoming_messages_internal(
                        &dlc_message_handler,
                        &dlc_man,
                        &sub_channel_manager,
                        &peer_manager,
                    )
                    .unwrap();
                    tokio::time::sleep(Duration::from_secs(30)).await;
                }
            }
        });

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(30)).await;

                // TODO: Fix unwraps: We need events so we can publish state of doing stuff

                let mut pending_trades = pending_trades.lock().unwrap();

                let mut to_be_deleted: HashSet<PublicKey> = HashSet::new();

                // TODO: this is not ideal as it is only needed on the coordinator
                for pubkey in pending_trades.iter() {
                    let pubkey_string = pubkey.to_string();
                    tracing::debug!(pubkey_string, "Checking for sub channel offers");
                    let sub_channels = dlc_man
                        .get_store()
                        .get_sub_channels() // `get_offered_sub_channels` appears to have a bug
                        .map_err(|e| anyhow!(e.to_string()))
                        .unwrap();

                    tracing::info!("Found sub_channels: {}", sub_channels.len());

                    let sub_channel = match sub_channels.iter().find(|sub_channel| {
                        dbg!(sub_channel.counter_party) == dbg!(*pubkey)
                            && matches!(&sub_channel.state, SubChannelState::Offered(_))
                    }) {
                        None => {
                            tracing::debug!(pubkey_string, "Nothing found for pubkey");
                            continue;
                        }
                        Some(sub_channel) => sub_channel,
                    };
                    to_be_deleted.insert(*pubkey);

                    let channel_id = sub_channel.channel_id;

                    let channel_id_hex = hex::encode(channel_id);

                    tracing::info!(channel_id = %channel_id_hex, "Accepting DLC channel offer");

                    let (node_id, accept_sub_channel) = sub_channel_manager
                        .accept_sub_channel(&channel_id)
                        .map_err(|e| anyhow!(e.to_string()))
                        .unwrap();

                    dlc_message_handler.send_subchannel_message(
                        node_id,
                        SubChannelMessage::Accept(accept_sub_channel),
                    );
                }

                for delete_me in to_be_deleted {
                    pending_trades.remove(&delete_me);
                }
            }
        });

        tracing::info!(
            "Lightning node started with node ID {}@{}",
            self.info.pubkey,
            self.info.address
        );
        Ok(background_processor)
    }

    /// Initiates the open private channel protocol.
    ///
    /// Returns a temporary channel ID as a 32-byte long array.
    pub fn initiate_open_channel(
        &self,
        peer: NodeInfo,
        channel_amount_sat: u64,
        initial_send_amount_sats: u64,
    ) -> Result<[u8; 32]> {
        let mut user_config = coordinator_config();
        user_config.channel_handshake_config.announced_channel = false;

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

        tracing::info!(
            %peer,
            temp_channel_id = %hex::encode(temp_channel_id),
            "Started channel creation"
        );

        Ok(temp_channel_id)
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

    /// Creates an invoice which is meant to be intercepted
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
            // the min final cltv defaults to 9, which does not work with lnd.
            // todo: Check why the value needs to be set to at least 20, as the payment will
            // otherwise fail with an error on lnd `Unable to send payment:
            // incorrect_payment_details`
            .min_final_cltv_expiry(20)
            .private_route(RouteHint(vec![RouteHintHop {
                src_node_id: hop_before_me,
                short_channel_id: intercepted_channel_id,
                // QUESTION: What happens if these differ with the actual values
                // in the `ChannelConfig` for the private channel?
                fees: RoutingFees {
                    base_msat: 1000,
                    proportional_millionths: LIQUIDITY_ROUTING_FEE_MILLIONTHS,
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

    pub async fn propose_dlc_channel(
        &self,
        channel_details: &ChannelDetails,
        contract_input: &DlcContractInput,
    ) -> Result<()> {
        let announcement = tokio::task::spawn_blocking({
            let oracle = self.oracle.clone();
            let event_id = contract_input.contract_infos[0].oracles.event_id.clone();
            move || {
                oracle
                    .get_announcement(&event_id)
                    .map_err(|e| anyhow!(e.to_string()))
            }
        })
        .await??;

        let sub_channel_offer = self
            .sub_channel_manager
            .offer_sub_channel(
                &channel_details.channel_id,
                contract_input,
                &[vec![announcement]],
            )
            .unwrap();

        self.dlc_message_handler.send_subchannel_message(
            channel_details.counterparty.node_id,
            SubChannelMessage::Request(sub_channel_offer),
        );

        Ok(())
    }

    pub fn initiate_accept_dlc_channel_offer(&self, channel_id: &[u8; 32]) -> Result<()> {
        let channel_id_hex = hex::encode(channel_id);

        tracing::info!(channel_id = %channel_id_hex, "Accepting DLC channel offer");

        let (node_id, accept_sub_channel) = self
            .sub_channel_manager
            .accept_sub_channel(channel_id)
            .map_err(|e| anyhow!(e.to_string()))?;

        self.dlc_message_handler
            .send_subchannel_message(node_id, SubChannelMessage::Accept(accept_sub_channel));

        Ok(())
    }

    pub fn process_incoming_messages(&self) -> Result<()> {
        Node::process_incoming_messages_internal(
            &self.dlc_message_handler,
            &self.dlc_manager,
            &self.sub_channel_manager,
            &self.peer_manager,
        )
    }

    fn process_incoming_messages_internal(
        dlc_message_handler: &DlcMessageHandler,
        dlc_manager: &DlcManager,
        sub_channel_manager: &SubChannelManager,
        peer_manager: &PeerManager,
    ) -> Result<(), Error> {
        let messages = dlc_message_handler.get_and_clear_received_messages();

        for (node_id, msg) in messages {
            tracing::debug!(from = %node_id, "Processing DLC-manager message");
            let resp = dlc_manager
                .on_dlc_message(&msg, node_id)
                .map_err(|e| anyhow!(e.to_string()))?;

            if let Some(msg) = resp {
                tracing::debug!(to = %node_id, "Sending DLC-manager message");
                dlc_message_handler.send_message(node_id, msg);
            }
        }

        let sub_channel_messages =
            dlc_message_handler.get_and_clear_received_sub_channel_messages();

        for (node_id, msg) in sub_channel_messages {
            tracing::debug!(
                from = %node_id,
                msg = %sub_channel_message_as_str(&msg),
                "Processing sub-channel message"
            );
            let resp = sub_channel_manager
                .on_sub_channel_message(&msg, &node_id)
                .map_err(|e| anyhow!(e.to_string()))?;

            if let Some(msg) = resp {
                tracing::debug!(
                    to = %node_id,
                    msg = %sub_channel_message_as_str(&msg),
                    "Sending sub-channel message"
                );
                dlc_message_handler.send_subchannel_message(node_id, msg);
            }
        }

        if dlc_message_handler.has_pending_messages() {
            peer_manager.process_events();
        }

        Ok(())
    }

    pub fn trade(&self, trade_params: TradeParams) -> Result<()> {
        let mut pending_trades = self.pending_trades.lock().unwrap();
        pending_trades.insert(trade_params.taker_node_pubkey);

        // TODO: Handle maker setup

        Ok(())
    }

    pub fn list_usable_channels(&self) -> Vec<ChannelDetails> {
        self.channel_manager.list_usable_channels()
    }

    pub fn get_contracts(&self) -> Result<Vec<Contract>> {
        self.dlc_manager
            .get_store()
            .get_contracts()
            .map_err(|e| anyhow!("Unable to get contracts from manager: {e:#}"))
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContractInput {}

#[derive(Debug, Clone, Deserialize)]
pub struct TradeParams {
    pub taker_node_pubkey: bdk::bitcoin::secp256k1::PublicKey,
    pub contract_input: ContractInput,
}

pub(crate) fn app_config() -> UserConfig {
    UserConfig {
        channel_handshake_config: ChannelHandshakeConfig {
            // The app will only accept private channels. As we are forcing the apps announced
            // channel preferences, the coordinator needs to override this config to match the apps
            // preferences.
            announced_channel: false,
            minimum_depth: 1,
            // only 10% of the total channel value can be sent. e.g. with a volume of 30.000 sats
            // only 3.000 sats can be sent.
            max_inbound_htlc_value_in_flight_percent_of_channel: 10,
            ..Default::default()
        },
        channel_handshake_limits: ChannelHandshakeLimits {
            max_minimum_depth: 1,
            trust_own_funding_0conf: true,
            // Enforces that incoming channels will be private.
            force_announced_channel_preference: true,
            // lnd's max to_self_delay is 2016, so we want to be compatible.
            their_to_self_delay: 2016,
            ..Default::default()
        },
        channel_config: ChannelConfig {
            cltv_expiry_delta: MIN_CLTV_EXPIRY_DELTA,
            ..Default::default()
        },
        // we want to accept 0-conf channels from the coordinator
        manually_accept_inbound_channels: true,
        ..Default::default()
    }
}

pub(crate) fn coordinator_config() -> UserConfig {
    UserConfig {
        channel_handshake_config: ChannelHandshakeConfig {
            // The coordinator will by default only accept public channels. (see also
            // force_announced_channel_preference). In order to open a private channel with the
            // mobile app this config gets overwritten during the creation of the just-in-time
            // channel)
            // Note, public channels need 6 confirmations to get announced (and usable for multi-hop
            // payments) this is a requirement of BOLT 7.
            announced_channel: true,
            // The minimum amount of confirmations before the inbound channel is deemed useable,
            // between the counterparties
            minimum_depth: 1,
            // only 10% of the total channel value can be sent. e.g. with a volume of 30.000 sats
            // only 3.000 sats can be sent.
            max_inbound_htlc_value_in_flight_percent_of_channel: 10,
            ..Default::default()
        },
        channel_handshake_limits: ChannelHandshakeLimits {
            // The minimum amount of confirmations before the outbound channel is deemed useable,
            // between the counterparties
            max_minimum_depth: 1,
            trust_own_funding_0conf: true,
            // Enforces incoming channels to the coordinator to be public! We
            // only want to open private channels to our 10101 app.
            force_announced_channel_preference: true,
            // lnd's max to_self_delay is 2016, so we want to be compatible.
            their_to_self_delay: 2016,
            ..Default::default()
        },
        channel_config: ChannelConfig {
            cltv_expiry_delta: MIN_CLTV_EXPIRY_DELTA,
            ..Default::default()
        },
        // This is needed to intercept payments to open just-in-time channels. This will produce the
        // HTLCIntercepted event.
        accept_intercept_htlcs: true,
        // This config is needed to forward payments to the 10101 app, which only have private
        // channels with the coordinator.
        accept_forwards_to_priv_channels: true,
        // the coordinator automatically accepts any inbound channels if the adhere to it's channel
        // preferences. (public, etc.)
        manually_accept_inbound_channels: false,
        ..Default::default()
    }
}

fn sub_channel_message_as_str(msg: &SubChannelMessage) -> &str {
    use SubChannelMessage::*;

    match msg {
        Request(_) => "Request",
        Accept(_) => "Accept",
        Confirm(_) => "Confirm",
        Finalize(_) => "Finalize",
        CloseOffer(_) => "CloseOffer",
        CloseAccept(_) => "CloseAccept",
        CloseConfirm(_) => "CloseConfirm",
        CloseFinalize(_) => "CloseFinalize",
        CloseReject(_) => "CloseReject",
    }
}
