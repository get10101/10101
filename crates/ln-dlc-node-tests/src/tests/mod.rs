use anyhow::Result;
use bitcoin::hashes::sha256;
use bitcoin::secp256k1::PublicKey;
use bitcoin::Amount;
use bitcoin::Network;
use bitcoin::XOnlyPublicKey;
use coordinator::config::coordinator_config;
use coordinator::node::coordinator_event_handler::CoordinatorEventHandler;
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
use dlc_messages::Message;
use futures::Future;
use lightning::ln::channelmanager::ChannelDetails;
use lightning::util::config::UserConfig;
use lightning::util::events::Event;
use ln_dlc_node::config::LIQUIDITY_MULTIPLIER;
use ln_dlc_node::node::sub_channel_message_name;
use ln_dlc_node::node::InMemoryStore;
use ln_dlc_node::node::LnDlcNodeSettings;
use ln_dlc_node::node::Node;
use ln_dlc_node::node::NodeInfo;
use ln_dlc_node::node::OracleInfo;
use ln_dlc_node::node::RunningNode;
use ln_dlc_node::scorer;
use ln_dlc_node::seed::Bip39Seed;
use ln_dlc_node::util;
use ln_dlc_node::EventHandlerTrait;
use ln_dlc_node::EventSender;
use ln_dlc_node::HTLCStatus;
use native::config::app_config;
use native::ln_dlc::app_event_handler::AppEventHandler;
use rand::distributions::Alphanumeric;
use rand::thread_rng;
use rand::Rng;
use rand::RngCore;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
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
mod dlc;
mod just_in_time_channel;
mod multi_hop_payment;
mod single_hop_payment;

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
                bdk=info,\
                lightning::ln::peer_handler=debug,\
                lightning=trace,\
                sled=info,\
                ureq=info",
            )
            .with_test_writer()
            .init()
    })
}

/// Wrapper over the ln_dlc_node::Node with some convenience methods for testing.
///
/// Implements Deref so that we can access the underlying Node without boilerplate.
pub struct TestNode(pub Arc<Node<InMemoryStore>>);

impl std::ops::Deref for TestNode {
    type Target = Arc<Node<InMemoryStore>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl TestNode {
    fn start_test_app(name: &str) -> Result<(Self, RunningNode)> {
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
            LnDlcNodeSettings::default(),
            None,
        )
    }

    fn start_test_coordinator(name: &str) -> Result<(Self, RunningNode)> {
        Self::start_test_coordinator_internal(
            name,
            Arc::new(InMemoryStore::default()),
            LnDlcNodeSettings::default(),
            None,
        )
    }

    fn start_test_coordinator_internal(
        name: &str,
        storage: Arc<InMemoryStore>,
        settings: LnDlcNodeSettings,
        ldk_event_sender: Option<watch::Sender<Option<Event>>>,
    ) -> Result<(Self, RunningNode)> {
        let max_app_channel_size_sats = settings.max_app_channel_size_sats;
        let coordinator_event_handler = |node, event_sender| {
            Arc::new(CoordinatorEventHandler::new(
                node,
                event_sender,
                max_app_channel_size_sats,
            )) as Arc<dyn EventHandlerTrait>
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
        storage: Arc<InMemoryStore>,
        settings: LnDlcNodeSettings,
        ldk_event_sender: Option<watch::Sender<Option<Event>>>,
    ) -> Result<(Self, RunningNode)>
    where
        EH: Fn(Arc<Node<InMemoryStore>>, Option<EventSender>) -> Arc<dyn EventHandlerTrait>,
    {
        let data_dir = random_tmp_dir().join(name);

        let seed = Bip39Seed::new().expect("A valid bip39 seed");

        let mut ephemeral_randomness = [0; 32];
        thread_rng().fill_bytes(&mut ephemeral_randomness);

        let address = {
            let listener = TcpListener::bind("0.0.0.0:0").unwrap();
            listener.local_addr().expect("To get a free local address")
        };

        let node = Node::new(
            ldk_config,
            scorer::in_memory_scorer,
            name,
            Network::Regtest,
            data_dir.as_path(),
            storage,
            address,
            address,
            util::into_net_addresses(address),
            esplora_origin,
            seed,
            ephemeral_randomness,
            settings,
            oracle.into(),
        )?;
        let node = Arc::new(node);

        let event_handler = event_handler_factory(node.clone(), ldk_event_sender);
        let running = node.start(event_handler)?;

        tracing::debug!(%name, info = %node.info, "Node started");

        Ok((TestNode(node), running))
    }

    /// Trigger on-chain wallet sync.
    ///
    /// We wrap the wallet sync with a `block_in_place` to avoid blocking the async task in
    /// `tokio::test`s.
    ///
    /// Because we use `block_in_place`, we must configure the `tokio::test`s with `flavor =
    /// "multi_thread"`.
    async fn sync_on_chain(&self) -> Result<()> {
        block_in_place(|| self.0.wallet().sync())
    }

    pub async fn wait_for_payment_claimed(
        &self,
        hash: &sha256::Hash,
    ) -> Result<(), tokio::time::error::Elapsed> {
        self.wait_for_payment(HTLCStatus::Succeeded, hash, None)
            .await
    }

    pub fn process_incoming_messages(&self) -> Result<()> {
        let dlc_message_handler = &self.dlc_message_handler;
        let dlc_manager = &self.dlc_manager;
        let sub_channel_manager = &self.sub_channel_manager;
        let messages = dlc_message_handler.get_and_clear_received_messages();
        tracing::debug!("Received and cleared {} messages", messages.len());

        for (node_id, msg) in messages {
            match msg {
                Message::OnChain(_) | Message::Channel(_) => {
                    tracing::debug!(from = %node_id, "Processing DLC-manager message");
                    let resp = dlc_manager.on_dlc_message(&msg, node_id)?;

                    if let Some(msg) = resp {
                        tracing::debug!(to = %node_id, "Sending DLC-manager message");
                        dlc_message_handler.send_message(node_id, msg);
                    }
                }
                Message::SubChannel(msg) => {
                    tracing::debug!(
                        from = %node_id,
                        msg = %sub_channel_message_name(&msg),
                        "Processing DLC channel message"
                    );
                    let resp = sub_channel_manager.on_sub_channel_message(&msg, &node_id)?;

                    if let Some(msg) = resp {
                        tracing::debug!(
                            to = %node_id,
                            msg = %sub_channel_message_name(&msg),
                            "Sending DLC channel message"
                        );
                        dlc_message_handler.send_message(node_id, Message::SubChannel(msg));
                    }
                }
            }
        }

        Ok(())
    }

    async fn fund(&self, amount: Amount) -> Result<()> {
        let starting_balance = self.get_confirmed_balance().await?;
        let expected_balance = starting_balance + amount.to_sat();

        // we mine blocks so that the internal wallet in bitcoind has enough utxos to fund the
        // wallet
        bitcoind::mine(11).await?;
        for _ in 0..10 {
            let address = self.0.wallet.unused_address();
            bitcoind::fund(address.to_string(), Amount::from_sat(amount.to_sat() / 10)).await?;
        }
        bitcoind::mine(1).await?;

        tokio::time::timeout(Duration::from_secs(30), async {
            while self.get_confirmed_balance().await.unwrap() < expected_balance {
                let interval = Duration::from_millis(200);

                self.sync_on_chain().await.unwrap();

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
        let balance = self.0.wallet.ldk_wallet().get_balance()?;

        Ok(balance.confirmed)
    }

    /// Initiates the opening of a private channel _and_ waits for the channel to be usable.
    async fn open_private_channel(
        &self,
        peer: &TestNode,
        amount_us: u64,
        amount_them: u64,
    ) -> Result<ChannelDetails> {
        self.open_channel(peer, amount_us, amount_them, false).await
    }

    /// Initiates the opening of a public channel _and_ waits for the channel to be usable.
    async fn open_public_channel(
        &self,
        peer: &TestNode,
        amount_us: u64,
        amount_them: u64,
    ) -> Result<ChannelDetails> {
        self.open_channel(peer, amount_us, amount_them, true).await
    }

    /// Initiates the opening of a channel _and_ waits for the channel to be usable.
    async fn open_channel(
        &self,
        peer: &TestNode,
        amount_us: u64,
        amount_them: u64,
        is_public: bool,
    ) -> Result<ChannelDetails> {
        let temp_channel_id = self.0.initiate_open_channel(
            peer.info.pubkey,
            amount_us + amount_them,
            amount_them,
            is_public,
        )?;

        let (does_manually_accept_inbound_channels, required_confirmations) =
            block_in_place(|| {
                let config = peer.0.ldk_config.read();

                (
                    config.manually_accept_inbound_channels,
                    config.channel_handshake_config.minimum_depth,
                )
            });

        // The config flag `channel_config.manually_accept_inbound_channels` implies that the peer
        // will accept 0-conf channels
        if !does_manually_accept_inbound_channels {
            bitcoind::mine(required_confirmations as u16).await?;
        }

        let channel_details = tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                if let Some(details) = self
                    .0
                    .channel_manager
                    .list_usable_channels()
                    .iter()
                    .find(|c| c.counterparty.node_id == peer.info.pubkey)
                {
                    break details.clone();
                }

                // Only sync if 0-conf channels are disabled
                if !does_manually_accept_inbound_channels {
                    // We need to sync both parties, even if
                    // `trust_own_funding_0conf` is true for the creator
                    // of the channel (`self`)
                    self.sync_on_chain().await.unwrap();
                    peer.sync_on_chain().await.unwrap();
                }

                tracing::debug!(
                    peer = %peer.info,
                    temp_channel_id = %hex::encode(temp_channel_id),
                    "Waiting for channel to be usable"
                );
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        })
        .await?;

        Ok(channel_details)
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
}

async fn setup_coordinator_payer_channel(
    payer_to_payee_invoice_amount: u64,
    coordinator: &TestNode,
    payer: &TestNode,
) -> u64 {
    let (
        coordinator_liquidity,
        coordinator_payer_inbound_liquidity,
        coordinator_payer_outbound_liquidity,
        expected_coordinator_payee_channel_value,
    ) = calculate_intercept_payment_values(payer_to_payee_invoice_amount);

    coordinator
        .fund(Amount::from_sat(coordinator_liquidity))
        .await
        .unwrap();

    coordinator
        .open_private_channel(
            payer,
            coordinator_payer_inbound_liquidity,
            coordinator_payer_outbound_liquidity,
        )
        .await
        .unwrap();

    expected_coordinator_payee_channel_value
}

fn calculate_intercept_payment_values(payer_to_payee_invoice_amount: u64) -> (u64, u64, u64, u64) {
    (
        // TODO: If we set the coordinator's liquidity precisely the test may fail reporting
        //  insufficient funds. This is likely because we don't pick up the change
        //  output correctly; further investigation needed why.

        // coordinator_liquidity
        // The invoice defines the channel value (invoice * liquidity_multiplier = channel value)
        // The coordinator has to provide liquidity with the payer and the payee.
        // For simplicity we give the coordinator a lot of liquidity to ensure the channels can be
        // opened.
        payer_to_payee_invoice_amount * LIQUIDITY_MULTIPLIER * 5,
        // coordinator_payer_inbound_liquidity
        // The liquidity of the coordinator in the coordinator<>payer channel
        // This has to be at least as much as the channel that will be opened from coordinator to
        // payee to allow payments between payer and payee. For simplicity we set this to
        // the amount that is equal to the channel that will be created between coordinator and
        // payee.
        payer_to_payee_invoice_amount * LIQUIDITY_MULTIPLIER,
        // coordinator_payer_outbound_liquidity
        // The liquidity of the payer in the coordinator<>payer channel
        // This has to be at least as much as the channel that will be opened from coordinator to
        // payee to allow payments between payer and payee. For simplicity we set this to
        // the amount that is equal to the channel that will be created between coordinator and
        // payee.
        payer_to_payee_invoice_amount * LIQUIDITY_MULTIPLIER,
        // expected_coordinator_payee_channel_value
        // The expected channel value of the channel between coordinator and payee.
        payer_to_payee_invoice_amount * LIQUIDITY_MULTIPLIER,
    )
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
fn log_channel_id(node: &Node<InMemoryStore>, index: usize, pair: &str) {
    let details = match node.channel_manager.list_channels().get(index) {
        Some(details) => details.clone(),
        None => {
            tracing::info!(%index, %pair, "No channel");
            return;
        }
    };

    let channel_id = hex::encode(details.channel_id);
    let short_channel_id = details.short_channel_id;
    let is_ready = details.is_channel_ready;
    let is_usable = details.is_usable;
    let inbound = details.inbound_capacity_msat;
    let outbound = details.outbound_capacity_msat;
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
                None => tokio::time::sleep(Duration::from_millis(500)).await,
            };
        }
    })
    .await?
}

async fn wait_until_dlc_channel_state(
    timeout: Duration,
    node: &TestNode,
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

/// Calculate the fee paid to route a payment through a node, in msat. That is, the difference
/// between the inbound HTLC and the outbound HTLC.
///
/// The `channel_config` is that of the routing node.
fn calculate_routing_fee_msat(
    channel_config: lightning::util::config::ChannelConfig,
    invoice_amount_sat: u64,
) -> u64 {
    let flat_fee_msat = Decimal::from(channel_config.forwarding_fee_base_msat);
    let forwarding_fee_millionths_of_a_sat_per_sat =
        Decimal::from(channel_config.forwarding_fee_proportional_millionths);

    let proportional_fee_msat_per_sat =
        forwarding_fee_millionths_of_a_sat_per_sat / Decimal::ONE_THOUSAND;
    let proportional_fee_msat = Decimal::from(invoice_amount_sat) * proportional_fee_msat_per_sat;

    (flat_fee_msat + proportional_fee_msat).to_u64().unwrap()
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
                event_id: "btcusd1610611200".to_string(),
                threshold: 1,
            },
        }],
    }
}
