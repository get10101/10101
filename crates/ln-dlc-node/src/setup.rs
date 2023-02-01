use crate::disk;
use crate::ln::event_handler::handle_event;
use crate::ChainMonitor;
use crate::ChannelManager;
use crate::PaymentInfoStorage;
use crate::PeerManager;
use crate::TracingLogger;
use bitcoin::blockdata::constants::genesis_block;
use bitcoin::Network;
use dlc_manager::custom_signer::CustomKeysManager;
use dlc_messages::message_handler::MessageHandler as DlcMessageHandler;
use dlc_sled_storage_provider::SledStorageProvider;
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
use lightning_block_sync::BlockSource;
use lightning_persister::FilesystemPersister;
use rand::thread_rng;
use rand::Rng;
use simple_wallet::SimpleWallet;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::SystemTime;

/// The pre-condition to calling this function is that the environment
/// is set up. I was thinking of `nigiri` here.
pub async fn start_ln_dlc_node(ln_listening_port: u32) {
    let logger = Arc::new(TracingLogger);
    let electrs_host = "http://localhost:30000/".to_string();
    let network = Network::Regtest;
    let ldk_data_dir = "./".to_string();

    let electrs = tokio::task::spawn_blocking(move || {
        Arc::new(ElectrsBlockchainProvider::new(electrs_host, network))
    })
    .await
    .unwrap();

    // TODO: Might be better to use an in-memory persister for the tests.
    let persister = Arc::new(FilesystemPersister::new(ldk_data_dir.clone()));

    let chain_monitor: Arc<ChainMonitor> = Arc::new(chainmonitor::ChainMonitor::new(
        None,
        electrs.clone(),
        logger.clone(),
        electrs.clone(),
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

    let mut ldk_user_config = lightning::util::config::UserConfig {
        channel_handshake_config: lightning::util::config::ChannelHandshakeConfig {
            max_inbound_htlc_value_in_flight_percent_of_channel: 50,
            ..Default::default()
        },
        ..Default::default()
    };
    ldk_user_config
        .channel_handshake_limits
        .force_announced_channel_preference = false;

    let (channel_manager_blockhash, channel_manager) = {
        let (block_hash, block_height) = electrs.get_best_block().await.unwrap();

        let chain_params = ChainParameters {
            network,
            best_block: BestBlock::new(block_hash, block_height.unwrap()),
        };
        let channel_manager = ChannelManager::new(
            electrs.clone(),
            chain_monitor.clone(),
            electrs.clone(),
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

    // TODO: Replace with BDK?
    let wallet = Arc::new(SimpleWallet::new(electrs.clone(), storage, network));

    let event_handler = {
        let channel_manager = channel_manager.clone();
        let electrs = electrs.clone();
        let network_graph = network_graph;
        let keys_manager = keys_manager.clone();

        move |event: lightning::util::events::Event| {
            handle.block_on(handle_event(
                &channel_manager,
                &electrs,
                &network_graph,
                keys_manager,
                &inbound_payments,
                &outbound_payments,
                &wallet,
                &event,
            ));
        }
    };

    // // Step 14: Connect and Disconnect Blocks
    // if chain_tip.is_none() {
    //     chain_tip = Some(
    //         init::validate_best_block_header(electrs.clone())
    //             .await
    //             .unwrap(),
    //     );
    // }
    // let chain_tip = chain_tip.unwrap();
    // let channel_manager_listener = channel_manager.clone();
    // let chain_monitor_listener = chain_monitor.clone();
    // let electrs_clone = electrs.clone();
    // let peer_manager_clone = peer_manager.clone();
    // let event_handler_clone = event_handler.clone();
    // let logger_clone = logger.clone();
    // std::thread::spawn(move || {
    //     let mut last_height = chain_tip.height as u64;
    //     loop {
    //         let chain_tip_height = electrs_clone.get_blockchain_height().unwrap();
    //         for i in last_height + 1..=chain_tip_height {
    //             let block = electrs_clone.get_block_at_height(i).unwrap();
    //             channel_manager_listener.block_connected(&block, i as u32);
    //             for ftxo in chain_monitor_listener.list_monitors() {
    //                 chain_monitor_listener
    //                     .get_monitor(ftxo)
    //                     .unwrap()
    //                     .block_connected(
    //                         &block.header,
    //                         &block.txdata.iter().enumerate().collect::<Vec<_>>(),
    //                         i as u32,
    //                         electrs_clone.clone(),
    //                         electrs_clone.clone(),
    //                         logger_clone.clone(),
    //                     );
    //             }
    //         }
    //         last_height = chain_tip_height;
    //         channel_manager_listener.process_pending_events(&event_handler_clone);
    //         chain_monitor_listener.process_pending_events(&event_handler_clone);
    //         peer_manager_clone.process_events();
    //         std::thread::sleep(Duration::from_secs(1));
    //     }
    // });

    // // Step 16: Initialize routing ProbabilisticScorer
    // let scorer_path = format!("{}/scorer", ldk_data_dir.clone());
    // let scorer = Arc::new(Mutex::new(disk::read_scorer(
    //     Path::new(&scorer_path),
    //     Arc::clone(&network_graph),
    //     Arc::clone(&logger),
    // )));

    // // Step 17: Create InvoicePayer
    // let router = DefaultRouter::new(
    //     network_graph.clone(),
    //     logger.clone(),
    //     keys_manager.get_secure_random_bytes(),
    //     scorer.clone(),
    // );
    // let invoice_payer = Arc::new(InvoicePayer::new(
    //     channel_manager.clone(),
    //     router,
    //     logger.clone(),
    //     event_handler.clone(),
    //     payment::Retry::Timeout(Duration::from_secs(10)),
    // ));

    // // Step 18: Persist ChannelManager and NetworkGraph
    // let persister = Arc::new(FilesystemPersister::new(ldk_data_dir.clone()));

    // // Step 19: Background Processing
    // let background_processor = BackgroundProcessor::start(
    //     persister,
    //     invoice_payer.clone(),
    //     chain_monitor.clone(),
    //     channel_manager.clone(),
    //     GossipSync::P2P(gossip_sync.clone()),
    //     peer_manager.clone(),
    //     logger.clone(),
    //     Some(scorer.clone()),
    // );

    // // Regularly reconnect to channel peers.
    // let connect_cm = Arc::clone(&channel_manager);
    // let connect_pm = Arc::clone(&peer_manager);
    // let peer_data_path = format!("{}/channel_peer_data", ldk_data_dir.clone());
    // let stop_connect = Arc::clone(&stop_listen_connect);
    // tokio::spawn(async move {
    //     let mut interval = tokio::time::interval(Duration::from_secs(1));
    //     loop {
    //         interval.tick().await;
    //         match disk::read_channel_peer_data(Path::new(&peer_data_path)) {
    //             Ok(info) => {
    //                 let peers = connect_pm.get_peer_node_ids();
    //                 for node_id in connect_cm
    //                     .list_channels()
    //                     .iter()
    //                     .map(|chan| chan.counterparty.node_id)
    //                     .filter(|id| !peers.contains(id))
    //                 {
    //                     if stop_connect.load(Ordering::Acquire) {
    //                         return;
    //                     }
    //                     for (pubkey, peer_addr) in info.iter() {
    //                         if *pubkey == node_id {
    //                             let _ = cli::do_connect_peer(
    //                                 *pubkey,
    //                                 peer_addr.clone(),
    //                                 Arc::clone(&connect_pm),
    //                             )
    //                             .await;
    //                         }
    //                     }
    //                 }
    //             }
    //             Err(e) => println!(
    //                 "ERROR: errored reading channel peer info from disk: {:?}",
    //                 e
    //             ),
    //         }
    //     }
    // });

    // // Regularly broadcast our node_announcement. This is only required (or possible) if we have
    // // some public channels, and is only useful if we have public listen address(es) to
    // // announce. In a production environment, this should occur only after the
    // // announcement of new channels to avoid churn in the global network graph.
    // let network = config.network;
    // // if !args.ldk_announced_listen_addr.is_empty() {
    // //      tokio::spawn(async move {
    // //              let mut interval = tokio::time::interval(Duration::from_secs(60));
    // //              loop {
    // //                      interval.tick().await;
    // //                      chan_manager.broadcast_node_announcement(
    // //                              [0; 3],
    // //                              args.ldk_announced_node_name,
    // //                              args.ldk_announced_listen_addr.clone(),
    // //                      );
    // //              }
    // //      });
    // // }

    // let p2pdoracle = tokio::task::spawn_blocking(move || {
    //     Arc::new(
    //         P2PDOracleClient::new("https://oracle.p2pderivatives.io/")
    //             .expect("to be able to create the p2pd oracle"),
    //     )
    // })
    // .await
    // .unwrap();

    // let oracle_pubkey = p2pdoracle.get_public_key();

    // let oracles = HashMap::from([(oracle_pubkey, p2pdoracle.clone())]);

    // let wallet_clone = wallet.clone();
    // let electrs_clone = electrs.clone();

    // let addresses = storage.get_addresses().unwrap();
    // for address in addresses {
    //     println!("{}", address);
    // }

    // let store_clone = storage.clone();

    // let dlc_manager = tokio::task::spawn_blocking(move || {
    //     Arc::new(
    //         Manager::new(
    //             wallet_clone,
    //             electrs_clone.clone(),
    //             store_clone,
    //             oracles,
    //             Arc::new(SystemTimeProvider {}),
    //             electrs_clone,
    //         )
    //         .unwrap(),
    //     )
    // })
    // .await
    // .unwrap();

    // let electrs_clone = electrs.clone();
    // let init_height =
    //     tokio::task::spawn_blocking(move || electrs_clone.get_blockchain_height().unwrap())
    //         .await
    //         .unwrap();

    // let sub_channel_manager = Arc::new(SubChannelManager::new(
    //     Secp256k1::new(),
    //     wallet.clone(),
    //     channel_manager.clone(),
    //     storage,
    //     electrs.clone(),
    //     dlc_manager.clone(),
    //     electrs.clone(),
    //     init_height,
    // ));

    // loop {
    //     println!("Enter 1 for LN functions, 2 for DLC, 3 for wallet");
    //     print!(">");
    //     io::stdout().flush().unwrap(); // Without flushing, the `>` doesn't print

    //     let mut input_line = String::new();
    //     io::stdin()
    //         .read_line(&mut input_line)
    //         .expect("Failed to read line");
    //     if input_line.is_empty() {
    //         break;
    //     }

    //     let choice = input_line.trim().parse();

    //     match choice {
    //         Err(_) => println!("Invalid input"),
    //         Ok(c) => match c {
    //             0 => break,
    //             1 => {
    //                 cli::poll_for_user_input(
    //                     Arc::clone(&invoice_payer),
    //                     Arc::clone(&peer_manager),
    //                     Arc::clone(&channel_manager),
    //                     Arc::clone(&keys_manager),
    //                     Arc::clone(&network_graph),
    //                     inbound_payments.clone(),
    //                     outbound_payments.clone(),
    //                     ldk_data_dir.clone(),
    //                     network,
    //                     logger.clone(),
    //                 )
    //                 .await
    //             }
    //             2 => {
    //                 dlc_cli::poll_for_user_input(
    //                     peer_manager.clone(),
    //                     dlc_message_handler.clone(),
    //                     dlc_manager.clone(),
    //                     sub_channel_manager.clone(),
    //                     p2pdoracle.clone(),
    //                     &offers_path,
    //                 )
    //                 .await
    //             }
    //             3 => {
    //                 wallet_cli::poll_for_user_input(&wallet).await;
    //             }
    //             _ => {
    //                 println!("Invalid choice");
    //             }
    //         },
    //     }
    // }

    // // Disconnect our peers and stop accepting new connections. This ensures we don't continue
    // // updating our channel data after we've stopped the background processor.
    // stop_listen_connect.store(true, Ordering::Release);
    // peer_manager.disconnect_all_peers();

    // // Stop the background processor.
    // background_processor.stop().unwrap();
}
