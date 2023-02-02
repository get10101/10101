use crate::bdk_ldk_wallet::BDKLDKWallet;
use crate::bdk_wallet::BDKWallet;
use crate::disk;
use crate::ln::event_handler::handle_event;
use crate::node::BdkLdkWallet;
use crate::ChainMonitor;
use crate::ChannelManager;
use crate::GossipSync;
use crate::InvoicePayer;
use crate::PaymentInfoStorage;
use crate::PeerManager;
use crate::TracingLogger;
use bdk::blockchain::ElectrumBlockchain;
use bdk::blockchain::GetBlockHash;
use bdk::blockchain::GetHeight;
use bdk::electrum_client::Client;
use bitcoin::blockdata::constants::genesis_block;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::Network;
use dlc_manager::custom_signer::CustomKeysManager;
use dlc_manager::manager::Manager;
use dlc_manager::sub_channel_manager::SubChannelManager;
use dlc_manager::Blockchain;
use dlc_manager::Oracle;
use dlc_manager::SystemTimeProvider;
use dlc_messages::message_handler::MessageHandler as DlcMessageHandler;
use dlc_sled_storage_provider::SledStorageProvider;
use electrs_blockchain_provider::ElectrsBlockchainProvider;
use lightning::chain;
use lightning::chain::chainmonitor;
use lightning::chain::keysinterface::KeysInterface;
use lightning::chain::keysinterface::KeysManager;
use lightning::chain::BestBlock;
use lightning::chain::Listen;
use lightning::ln::channelmanager::ChainParameters;
use lightning::ln::peer_handler::IgnoringMessageHandler;
use lightning::ln::peer_handler::MessageHandler;
use lightning::routing::gossip::P2PGossipSync;
use lightning::routing::router::DefaultRouter;
use lightning::util::config::ChannelHandshakeLimits;
use lightning::util::events::EventsProvider;
use lightning_background_processor::BackgroundProcessor;
use lightning_block_sync::init;
use lightning_block_sync::poll;
use lightning_block_sync::BlockSource;
use lightning_invoice::payment;
use lightning_persister::FilesystemPersister;
use rand::thread_rng;
use rand::RngCore;
use simple_wallet::SimpleWallet;
use simple_wallet::WalletStorage;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::time::SystemTime;

/// The pre-condition to calling this function is that the environment
/// is set up. I was thinking of `nigiri` here.
async fn start_ln_dlc_node(
    ln_listening_port: u32,
    oracle_client: impl Oracle + Send + Sync + 'static,
    ldk_data_subdir: &str,
) {
    let logger = Arc::new(TracingLogger);
    let electrs_host = "http://localhost:30000/".to_string();
    let network = Network::Regtest;
    let ldk_data_dir = format!("./.ldk-data/{ldk_data_subdir}");

    let client = Client::new(&electrs_host.clone()).unwrap();
    let blockchain = ElectrumBlockchain::from(client);

    let path = Path::new(&ldk_data_dir);
    let bdk_wallet = BDKWallet::new(path, network).unwrap();
    let lightning_wallet = Arc::new(bdk_ldk::LightningWallet::new(
        Box::new(blockchain),
        bdk_wallet.inner,
    ));

    // TODO: Might be better to use an in-memory persister for the tests.
    let persister = Arc::new(FilesystemPersister::new(ldk_data_dir.clone()));

    let chain_monitor: Arc<ChainMonitor> = Arc::new(chainmonitor::ChainMonitor::new(
        None,
        lightning_wallet.clone(),
        logger.clone(),
        lightning_wallet.clone(),
        persister.clone(),
    ));

    let keys_manager = {
        // TODO: Pass this as an argument?
        let key = {
            let mut key = [0; 32];
            thread_rng().fill_bytes(&mut key);
            key
        };

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap();

        let keys_manager = KeysManager::new(&key, now.as_secs(), now.subsec_nanos());
        Arc::new(CustomKeysManager::new(keys_manager))
    };

    let channelmonitors = persister
        .read_channelmonitors(keys_manager.clone())
        .unwrap();

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

    let (channel_manager_blockhash, channel_manager) = {
        let chain_tip = lightning_wallet.get_tip().unwrap();
        let block_height = chain_tip.0;
        let block_hash = chain_tip.1.block_hash();

        let chain_params = ChainParameters {
            network,
            best_block: BestBlock::new(block_hash, block_height),
        };
        let channel_manager = ChannelManager::new(
            lightning_wallet.clone(),
            chain_monitor.clone(),
            lightning_wallet.clone(),
            logger.clone(),
            keys_manager.clone(),
            ldk_user_config,
            chain_params,
        );
        (block_hash, channel_manager)
    };

    println!("INFO: Our Node ID: {}", channel_manager.get_our_node_id());

    // Step 9: Sync ChannelMonitors and ChannelManager to chain tip
    // let mut chain_tip: Option<lightning_block_sync::poll::ValidatedBlockHeader> = None;

    // Step 11: Optional: Initialize the P2PGossipSync
    let genesis = genesis_block(network).header.block_hash();
    let network_graph_path = format!("{}/network_graph", ldk_data_dir.clone());
    let network_graph = Arc::new(disk::read_network(
        Path::new(&network_graph_path),
        genesis,
        logger.clone(),
    ));
    let gossip_sync = Arc::new(P2PGossipSync::new(
        Arc::clone(&network_graph),
        None::<Arc<dyn chain::Access + Send + Sync>>,
        logger.clone(),
    ));

    println!("INFO: Init P2PGossipSync");

    // Step 12: Initialize the PeerManager
    let channel_manager: Arc<ChannelManager> = Arc::new(channel_manager);
    let mut ephemeral_bytes = [0; 32];
    rand::thread_rng().fill_bytes(&mut ephemeral_bytes);
    let lightning_msg_handler = MessageHandler {
        chan_handler: channel_manager.clone(),
        route_handler: gossip_sync.clone(),
        onion_message_handler: Arc::new(IgnoringMessageHandler {}),
    };
    let dlc_message_handler = Arc::new(DlcMessageHandler::new());
    let current_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let peer_manager: Arc<PeerManager> = Arc::new(PeerManager::new(
        lightning_msg_handler,
        keys_manager
            .get_node_secret(chain::keysinterface::Recipient::Node)
            .unwrap(),
        current_time.try_into().unwrap(),
        &ephemeral_bytes,
        logger.clone(),
        dlc_message_handler,
    ));

    println!("INFO: Init PeerManager");

    // ## Running LDK
    // Step 13: Initialize networking

    tokio::spawn({
        let peer_manager = peer_manager.clone();
        async move {
            let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", ln_listening_port))
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

    println!("INFO: LN listener started");

    // Step 15: Handle LDK Events

    // TODO: persist payment info to disk
    let inbound_payments: PaymentInfoStorage = Arc::new(Mutex::new(HashMap::new()));
    let outbound_payments: PaymentInfoStorage = Arc::new(Mutex::new(HashMap::new()));
    let inbound_pmts_for_events = inbound_payments.clone();

    let handle = tokio::runtime::Handle::current();

    let storage = Arc::new(
        SledStorageProvider::new(&ldk_data_dir).expect("to be able to create sled storage"),
    );

    let event_handler = {
        let channel_manager = channel_manager.clone();
        let wallet = lightning_wallet.clone();
        let network_graph = network_graph.clone();
        let keys_manager = keys_manager.clone();

        move |event: lightning::util::events::Event| {
            handle.block_on(handle_event(
                &channel_manager,
                &wallet,
                &network_graph,
                keys_manager.clone(),
                &inbound_payments,
                &outbound_payments,
                &event,
            ));
        }
    };

    // Step 14: Connect and Disconnect Blocks
    /* TODO: we did not do this in the poc. However validating the latest block, seems to be a good idea? skipping for now as more thought is required here.

    let mut chain_tip: Option<poll::ValidatedBlockHeader> = None;
    if chain_tip.is_none() {
        chain_tip = Some(
            init::validate_best_block_header(blockchain.clone())
                .await
                .unwrap(),
        );
    }

    let chain_tip = chain_tip.unwrap();
    let channel_manager_listener = channel_manager.clone();
    let chain_monitor_listener = chain_monitor.clone();
    let electrs_clone = lightning_wallet.clone();
    let peer_manager_clone = peer_manager.clone();
    let event_handler_clone = event_handler.clone();
    let logger_clone = logger.clone();
    std::thread::spawn(move || {
        let mut last_height = chain_tip.height as u64;
        loop {
            let chain_tip_height = electrs_clone.get_blockchain_height().unwrap();
            for i in last_height + 1..=chain_tip_height {
                let block = electrs_clone.get_block_at_height(i).unwrap();
                channel_manager_listener.block_connected(&block, i as u32);
                for ftxo in chain_monitor_listener.list_monitors() {
                    chain_monitor_listener
                        .get_monitor(ftxo)
                        .unwrap()
                        .block_connected(
                            &block.header,
                            &block.txdata.iter().enumerate().collect::<Vec<_>>(),
                            i as u32,
                            electrs_clone.clone(),
                            electrs_clone.clone(),
                            logger_clone.clone(),
                        );
                }
            }
            last_height = chain_tip_height;
            channel_manager_listener.process_pending_events(&event_handler_clone);
            chain_monitor_listener.process_pending_events(&event_handler_clone);
            peer_manager_clone.process_events();
            std::thread::sleep(Duration::from_secs(1));
        }
    });

    */

    // Step 16: Initialize routing ProbabilisticScorer
    let scorer_path = format!("{}/scorer", ldk_data_dir.clone());
    let scorer = Arc::new(Mutex::new(disk::read_scorer(
        Path::new(&scorer_path),
        Arc::clone(&network_graph),
        Arc::clone(&logger),
    )));

    // Step 17: Create InvoicePayer
    let router = DefaultRouter::new(
        network_graph.clone(),
        logger.clone(),
        keys_manager.get_secure_random_bytes(),
        scorer.clone(),
    );
    let invoice_payer = Arc::new(InvoicePayer::new(
        channel_manager.clone(),
        router,
        logger.clone(),
        event_handler.clone(),
        payment::Retry::Timeout(Duration::from_secs(10)),
    ));

    // Step 18: Persist ChannelManager and NetworkGraph
    let persister = Arc::new(FilesystemPersister::new(ldk_data_dir.clone()));

    // Step 19: Background Processing
    let background_processor = BackgroundProcessor::start(
        persister,
        invoice_payer.clone(),
        chain_monitor.clone(),
        channel_manager.clone(),
        GossipSync::P2P(gossip_sync.clone()),
        peer_manager.clone(),
        logger.clone(),
        Some(scorer.clone()),
    );

    // TODO: Regularly reconnect to channel peers.

    let oracle_client = tokio::task::spawn_blocking(move || Arc::new(oracle_client))
        .await
        .unwrap();

    let oracle_pubkey = oracle_client.get_public_key();

    let oracles = HashMap::from([(oracle_pubkey, oracle_client.clone())]);

    let addresses = storage.get_addresses().unwrap();
    for address in addresses {
        println!("{}", address);
    }

    let store_clone = storage.clone();

    let electrs = tokio::task::spawn_blocking(move || {
        Arc::new(ElectrsBlockchainProvider::new(electrs_host, network))
    })
    .await
    .unwrap();

    let wallet = Arc::new(SimpleWallet::new(electrs.clone(), storage.clone(), network));

    let wallet_clone = wallet.clone();
    let lightning_wallet_clone = lightning_wallet.clone();
    let electrs_clone = electrs.clone();
    let dlc_manager = tokio::task::spawn_blocking(move || {
        Arc::new(
            Manager::new(
                wallet_clone,  // TODO: replace with bdk wallet
                electrs_clone, // TODO: blockchain can not be cloned
                store_clone,
                oracles,
                Arc::new(SystemTimeProvider {}),
                lightning_wallet_clone,
            )
            .unwrap(),
        )
    })
    .await
    .unwrap();

    let init_height = lightning_wallet.get_tip().unwrap().0;

    let bdk_ldk_wallet = BDKLDKWallet {
        inner: lightning_wallet.clone(),
    };

    let sub_channel_manager = Arc::new(SubChannelManager::new(
        Secp256k1::new(),
        wallet.clone(), // TODO: replace with bdk wallet
        channel_manager.clone(),
        storage,
        electrs.clone(), // TODO: replace with blockchain
        dlc_manager.clone(),
        lightning_wallet.clone(),
        init_height as u64,
    ));

    peer_manager.disconnect_all_peers();

    // Stop the background processor.
    background_processor.stop().unwrap();
}
