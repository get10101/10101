use crate::config::app_config;
use crate::config::coordinator_config;
use crate::node::event::NodeEventHandler;
use crate::node::peer_manager::alias_as_bytes;
use crate::node::GossipSourceConfig;
use crate::node::InMemoryStore;
use crate::node::LnDlcNodeSettings;
use crate::node::Node;
use crate::node::NodeInfo;
use crate::node::OracleInfo;
use crate::node::RunningNode;
use crate::scorer;
use crate::seed::Bip39Seed;
use crate::storage::TenTenOneInMemoryStorage;
use crate::util;
use crate::AppEventHandler;
use crate::CoordinatorEventHandler;
use crate::EventHandlerTrait;
use crate::EventSender;
use crate::WalletSettings;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use bitcoin::Amount;
use bitcoin::Network;
use bitcoin::XOnlyPublicKey;
use dlc_manager::contract::contract_input::ContractInput;
use dlc_manager::contract::contract_input::ContractInputInfo;
use dlc_manager::contract::contract_input::OracleInput;
use dlc_manager::contract::numerical_descriptor::NumericalDescriptor;
use dlc_manager::contract::ContractDescriptor;
use dlc_manager::payout_curve::PayoutFunction;
use dlc_manager::payout_curve::PayoutFunctionPiece;
use dlc_manager::payout_curve::PayoutPoint;
use dlc_manager::payout_curve::PolynomialPayoutCurvePiece;
use dlc_manager::payout_curve::RoundingInterval;
use dlc_manager::payout_curve::RoundingIntervals;
use dlc_manager::subchannel::SubChannel;
use dlc_manager::subchannel::SubChannelState;
use dlc_manager::Storage;
use futures::Future;
use lightning::events::Event;
use lightning::util::config::UserConfig;
use rand::distributions::Alphanumeric;
use rand::thread_rng;
use rand::Rng;
use rand::RngCore;
use std::env::temp_dir;
use std::net::TcpListener;
use std::path::PathBuf;
use std::str::FromStr;
use std::string::ToString;
use std::sync::Arc;
use std::sync::Once;
use std::time::Duration;
use tokio::sync::watch;
use tokio::task::block_in_place;

mod bitcoind;
mod dlc_channel;

#[cfg(feature = "load_tests")]
mod load;

const ESPLORA_ORIGIN: &str = "http://localhost:3000";
const FAUCET_ORIGIN: &str = "http://localhost:8080";
const ORACLE_ORIGIN: &str = "http://localhost:8081";
const ORACLE_PUBKEY: &str = "16f88cf7d21e6c0f46bcbc983a4e3b19726c6c98858cc31c83551a88fde171c0";

fn init_tracing() {
    static TRACING_TEST_SUBSCRIBER: Once = Once::new();

    TRACING_TEST_SUBSCRIBER.call_once(|| {
        tracing_subscriber::fmt()
            .with_env_filter(
                "debug,\
                hyper=warn,\
                reqwest=warn,\
                rustls=warn,\
                bdk=debug,\
                lightning::ln::peer_handler=debug,\
                lightning=trace,\
                lightning_transaction_sync=warn,\
                sled=info,\
                ureq=info",
            )
            .with_test_writer()
            .init()
    })
}

#[cfg(test)]
async fn wait_until_sub_channel_state(
    timeout: Duration,
    node: &Node<TenTenOneInMemoryStorage, InMemoryStore>,
    counterparty_pk: PublicKey,
    target_state: SubChannelStateName,
) -> Result<SubChannel> {
    wait_until(timeout, || async {
        node.process_incoming_messages()?;

        let dlc_channels = node.dlc_manager.get_store().get_sub_channels()?;

        Ok(dlc_channels
            .iter()
            .find(|channel| {
                let current_state = SubChannelStateName::from(&channel.state);

                tracing::info!(
                    node_id = %node.info.pubkey,
                    target = ?target_state,
                    current = ?current_state,
                    "Waiting for DLC subchannel to reach state"
                );

                channel.counter_party == counterparty_pk && current_state == target_state
            })
            .cloned())
    })
    .await
}

impl Node<TenTenOneInMemoryStorage, InMemoryStore> {
    fn start_test_app(name: &str) -> Result<(Arc<Self>, RunningNode)> {
        let app_event_handler = |node, event_sender| {
            Arc::new(AppEventHandler::new(node, event_sender)) as Arc<dyn EventHandlerTrait>
        };

        Self::start_test(
            app_event_handler,
            name,
            app_config(),
            ESPLORA_ORIGIN.to_string(),
            OracleInfo {
                endpoint: ORACLE_ORIGIN.to_string(),
                public_key: XOnlyPublicKey::from_str(ORACLE_PUBKEY)?,
            },
            Arc::new(InMemoryStore::default()),
            ln_dlc_node_settings_app(),
            None,
        )
    }

    fn start_test_coordinator(name: &str) -> Result<(Arc<Self>, RunningNode)> {
        Self::start_test_coordinator_internal(
            name,
            Arc::new(InMemoryStore::default()),
            ln_dlc_node_settings_coordinator(),
            None,
        )
    }

    fn start_test_coordinator_internal(
        name: &str,
        storage: Arc<InMemoryStore>,
        settings: LnDlcNodeSettings,
        ldk_event_sender: Option<watch::Sender<Option<Event>>>,
    ) -> Result<(Arc<Self>, RunningNode)> {
        let coordinator_event_handler = |node, event_sender| {
            Arc::new(CoordinatorEventHandler::new(node, event_sender)) as Arc<dyn EventHandlerTrait>
        };

        Self::start_test(
            coordinator_event_handler,
            name,
            coordinator_config(),
            ESPLORA_ORIGIN.to_string(),
            OracleInfo {
                endpoint: ORACLE_ORIGIN.to_string(),
                public_key: XOnlyPublicKey::from_str(ORACLE_PUBKEY)?,
            },
            storage,
            settings,
            ldk_event_sender,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn start_test<EH>(
        event_handler_factory: EH,
        name: &str,
        ldk_config: UserConfig,
        esplora_origin: String,
        oracle: OracleInfo,
        node_storage: Arc<InMemoryStore>,
        settings: LnDlcNodeSettings,
        ldk_event_sender: Option<watch::Sender<Option<Event>>>,
    ) -> Result<(Arc<Self>, RunningNode)>
    where
        EH: Fn(
            Arc<Node<TenTenOneInMemoryStorage, InMemoryStore>>,
            Option<EventSender>,
        ) -> Arc<dyn EventHandlerTrait>,
    {
        let data_dir = random_tmp_dir().join(name);

        let seed = Bip39Seed::new().expect("A valid bip39 seed");

        let mut ephemeral_randomness = [0; 32];
        thread_rng().fill_bytes(&mut ephemeral_randomness);

        let address = {
            let listener = TcpListener::bind("0.0.0.0:0").unwrap();
            listener.local_addr().expect("To get a free local address")
        };

        let storage = TenTenOneInMemoryStorage::new();

        let node = Node::new(
            ldk_config,
            scorer::in_memory_scorer,
            name,
            Network::Regtest,
            data_dir.as_path(),
            storage,
            node_storage,
            address,
            address,
            util::into_socket_addresses(address),
            esplora_origin,
            seed,
            ephemeral_randomness,
            settings,
            WalletSettings::default(),
            vec![oracle.into()],
            XOnlyPublicKey::from_str(ORACLE_PUBKEY)?,
            Arc::new(NodeEventHandler::new()),
        )?;
        let node = Arc::new(node);

        let event_handler = event_handler_factory(node.clone(), ldk_event_sender);
        let running = node.start(event_handler, false)?;

        tracing::debug!(%name, info = %node.info, "Node started");

        Ok((node, running))
    }

    /// Trigger on-chain and off-chain wallet syncs.
    ///
    /// We wrap the wallet sync with a `block_in_place` to avoid blocking the async task in
    /// `tokio::test`s.
    ///
    /// Because we use `block_in_place`, we must configure the `tokio::test`s with `flavor =
    /// "multi_thread"`.
    async fn sync_wallets(&self) -> Result<()> {
        block_in_place(|| {
            self.sync_on_chain_wallet()?;
            self.sync_lightning_wallet()?;

            Ok(())
        })
    }

    async fn fund(&self, amount: Amount) -> Result<()> {
        let starting_balance = self.get_confirmed_balance().await?;
        let expected_balance = starting_balance + amount.to_sat();

        // we mine blocks so that the internal wallet in bitcoind has enough utxos to fund the
        // wallet
        bitcoind::mine(11).await?;
        for _ in 0..10 {
            let address = self.wallet.unused_address();
            bitcoind::fund(address.to_string(), Amount::from_sat(amount.to_sat() / 10)).await?;
        }
        bitcoind::mine(1).await?;

        tokio::time::timeout(Duration::from_secs(30), async {
            while self.get_confirmed_balance().await.unwrap() < expected_balance {
                let interval = Duration::from_millis(200);

                self.sync_wallets().await.unwrap();

                tokio::time::sleep(interval).await;
                tracing::debug!(
                    ?interval,
                    "Checking if wallet has been funded after interval"
                );
            }
        })
        .await?;

        Ok(())
    }

    async fn get_confirmed_balance(&self) -> Result<u64> {
        let balance = self.wallet.ldk_wallet().get_balance()?;

        Ok(balance.confirmed)
    }

    pub fn disconnect(&self, peer: NodeInfo) {
        self.peer_manager.disconnect_by_node_id(peer.pubkey)
    }

    pub async fn reconnect(&self, peer: NodeInfo) -> Result<()> {
        self.disconnect(peer);
        tokio::time::sleep(Duration::from_secs(1)).await;
        self.connect(peer).await?;
        Ok(())
    }

    pub fn broadcast_node_announcement(&self) {
        let alias = alias_as_bytes(&self.alias).expect("alias to be the right length");

        crate::node::peer_manager::broadcast_node_announcement(
            &self.peer_manager,
            alias,
            self.announcement_addresses.clone(),
        );
    }
}

fn random_tmp_dir() -> PathBuf {
    let tmp = if let Ok(tmp) = std::env::var("RUNNER_TEMP") {
        tracing::debug!("Running test on github actions - using temporary directory at {tmp}");
        PathBuf::from(tmp)
    } else {
        temp_dir()
    };

    let rand_string = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(10)
        .map(char::from)
        .collect::<String>();

    let tmp = tmp.join(rand_string);

    tracing::debug!(
        path = %tmp.to_str().expect("to be a valid path"),
        "Generated temporary directory string"
    );

    tmp
}

#[allow(dead_code)]
fn log_channel_id(node: &Node<TenTenOneInMemoryStorage, InMemoryStore>, index: usize, pair: &str) {
    let details = match node.channel_manager.list_channels().get(index) {
        Some(details) => details.clone(),
        None => {
            tracing::info!(%index, %pair, "No channel");
            return;
        }
    };

    let channel_id = hex::encode(details.channel_id.0);
    let short_channel_id = details.short_channel_id;
    let is_ready = details.is_channel_ready;
    let is_usable = details.is_usable;
    let inbound = details.inbound_capacity_msat / 1000;
    let outbound = details.outbound_capacity_msat / 1000;
    tracing::info!(
        channel_id,
        short_channel_id,
        is_ready,
        is_usable,
        inbound,
        outbound,
        "{pair}"
    );
}

async fn wait_until<P, T, F>(timeout: Duration, predicate_fn: P) -> Result<T>
where
    P: Fn() -> F,
    F: Future<Output = Result<Option<T>>>,
{
    tokio::time::timeout(timeout, async {
        loop {
            match predicate_fn().await? {
                Some(value) => return Ok(value),
                None => tokio::time::sleep(Duration::from_millis(100)).await,
            };
        }
    })
    .await?
}

#[derive(PartialEq, Debug)]
enum SubChannelStateName {
    Offered,
    Accepted,
    Confirmed,
    Finalized,
    Signed,
    Closing,
    OnChainClosed,
    CounterOnChainClosed,
    CloseOffered,
    CloseAccepted,
    CloseConfirmed,
    OffChainClosed,
    ClosedPunished,
    Rejected,
}

impl From<&SubChannelState> for SubChannelStateName {
    fn from(value: &SubChannelState) -> Self {
        use SubChannelState::*;
        match value {
            Offered(_) => SubChannelStateName::Offered,
            Accepted(_) => SubChannelStateName::Accepted,
            Confirmed(_) => SubChannelStateName::Confirmed,
            Finalized(_) => SubChannelStateName::Finalized,
            Signed(_) => SubChannelStateName::Signed,
            Closing(_) => SubChannelStateName::Closing,
            OnChainClosed => SubChannelStateName::OnChainClosed,
            CounterOnChainClosed => SubChannelStateName::CounterOnChainClosed,
            CloseOffered(_) => SubChannelStateName::CloseOffered,
            CloseAccepted(_) => SubChannelStateName::CloseAccepted,
            CloseConfirmed(_) => SubChannelStateName::CloseConfirmed,
            OffChainClosed => SubChannelStateName::OffChainClosed,
            ClosedPunished(_) => SubChannelStateName::ClosedPunished,
            Rejected => SubChannelStateName::Rejected,
        }
    }
}

fn ln_dlc_node_settings_coordinator() -> LnDlcNodeSettings {
    LnDlcNodeSettings {
        off_chain_sync_interval: Duration::from_secs(5),
        on_chain_sync_interval: Duration::from_secs(300),
        fee_rate_sync_interval: Duration::from_secs(20),
        dlc_manager_periodic_check_interval: Duration::from_secs(30),
        sub_channel_manager_periodic_check_interval: Duration::from_secs(30),
        shadow_sync_interval: Duration::from_secs(600),
        forwarding_fee_proportional_millionths: 50,
        bdk_client_stop_gap: 20,
        bdk_client_concurrency: 4,
        gossip_source_config: GossipSourceConfig::P2pNetwork,
    }
}

fn ln_dlc_node_settings_app() -> LnDlcNodeSettings {
    LnDlcNodeSettings {
        off_chain_sync_interval: Duration::from_secs(5),
        on_chain_sync_interval: Duration::from_secs(300),
        fee_rate_sync_interval: Duration::from_secs(20),
        dlc_manager_periodic_check_interval: Duration::from_secs(30),
        sub_channel_manager_periodic_check_interval: Duration::from_secs(30),
        shadow_sync_interval: Duration::from_secs(600),
        forwarding_fee_proportional_millionths: 50,
        bdk_client_stop_gap: 20,
        bdk_client_concurrency: 4,
        gossip_source_config: GossipSourceConfig::P2pNetwork,
    }
}

fn dummy_contract_input(
    offer_collateral: u64,
    accept_collateral: u64,
    oracle_pk: XOnlyPublicKey,
) -> ContractInput {
    let total_collateral = offer_collateral + accept_collateral;

    let n_cets = 100;
    let rounding_mod = total_collateral / (n_cets + 1);

    ContractInput {
        offer_collateral,
        accept_collateral,
        fee_rate: 2,
        contract_infos: vec![ContractInputInfo {
            contract_descriptor: ContractDescriptor::Numerical(NumericalDescriptor {
                payout_function: PayoutFunction::new(vec![
                    PayoutFunctionPiece::PolynomialPayoutCurvePiece(
                        PolynomialPayoutCurvePiece::new(vec![
                            PayoutPoint {
                                event_outcome: 0,
                                outcome_payout: 0,
                                extra_precision: 0,
                            },
                            PayoutPoint {
                                event_outcome: 50_000,
                                outcome_payout: 0,
                                extra_precision: 0,
                            },
                        ])
                        .unwrap(),
                    ),
                    PayoutFunctionPiece::PolynomialPayoutCurvePiece(
                        PolynomialPayoutCurvePiece::new(vec![
                            PayoutPoint {
                                event_outcome: 50_000,
                                outcome_payout: 0,
                                extra_precision: 0,
                            },
                            PayoutPoint {
                                event_outcome: 60_000,
                                outcome_payout: total_collateral,
                                extra_precision: 0,
                            },
                        ])
                        .unwrap(),
                    ),
                    PayoutFunctionPiece::PolynomialPayoutCurvePiece(
                        PolynomialPayoutCurvePiece::new(vec![
                            PayoutPoint {
                                event_outcome: 60_000,
                                outcome_payout: total_collateral,
                                extra_precision: 0,
                            },
                            PayoutPoint {
                                event_outcome: 1048575,
                                outcome_payout: total_collateral,
                                extra_precision: 0,
                            },
                        ])
                        .unwrap(),
                    ),
                ])
                .unwrap(),
                rounding_intervals: RoundingIntervals {
                    intervals: vec![RoundingInterval {
                        begin_interval: 0,
                        rounding_mod,
                    }],
                },
                difference_params: None,
                oracle_numeric_infos: dlc_trie::OracleNumericInfo {
                    base: 2,
                    nb_digits: vec![20],
                },
            }),
            oracles: OracleInput {
                public_keys: vec![oracle_pk],
                event_id: "btcusd1706899460".to_string(),
                threshold: 1,
            },
        }],
    }
}
