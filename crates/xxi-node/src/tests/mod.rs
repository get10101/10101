use crate::bitcoin_conversion::to_secp_pk_29;
use crate::bitcoin_conversion::to_xonly_pk_29;
use crate::node::dlc_channel::send_dlc_message;
use crate::node::event::NodeEvent;
use crate::node::event::NodeEventHandler;
use crate::node::InMemoryStore;
use crate::node::Node;
use crate::node::NodeInfo;
use crate::node::OracleInfo;
use crate::node::RunningNode;
use crate::node::XXINodeSettings;
use crate::on_chain_wallet;
use crate::seed::Bip39Seed;
use crate::storage::DlcChannelEvent;
use crate::storage::TenTenOneInMemoryStorage;
use anyhow::Result;
use bitcoin::secp256k1::XOnlyPublicKey;
use bitcoin::Amount;
use bitcoin::Network;
use bitcoin_old::hashes::hex::ToHex;
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
use dlc_manager::ReferenceId;
use futures::Future;
use rand::distributions::Alphanumeric;
use rand::thread_rng;
use rand::Rng;
use rand::RngCore;
use std::collections::HashMap;
use std::env::temp_dir;
use std::net::TcpListener;
use std::path::PathBuf;
use std::str::FromStr;
use std::string::ToString;
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::Once;
use std::time::Duration;
use time::OffsetDateTime;
use uuid::Uuid;

mod bitcoind;
mod dlc_channel;

const ELECTRS_ORIGIN: &str = "http://localhost:3000";
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

impl Node<on_chain_wallet::InMemoryStorage, TenTenOneInMemoryStorage, InMemoryStore> {
    fn start_test_app(name: &str) -> Result<(Arc<Self>, RunningNode)> {
        Self::start_test(
            name,
            ELECTRS_ORIGIN.to_string(),
            OracleInfo {
                endpoint: ORACLE_ORIGIN.to_string(),
                public_key: XOnlyPublicKey::from_str(ORACLE_PUBKEY)?,
            },
            Arc::new(InMemoryStore::default()),
            xxi_node_settings_app(),
        )
    }

    fn start_test_coordinator(name: &str) -> Result<(Arc<Self>, RunningNode)> {
        Self::start_test_coordinator_internal(
            name,
            Arc::new(InMemoryStore::default()),
            xxi_node_settings_coordinator(),
        )
    }

    fn start_test_coordinator_internal(
        name: &str,
        storage: Arc<InMemoryStore>,
        settings: XXINodeSettings,
    ) -> Result<(Arc<Self>, RunningNode)> {
        Self::start_test(
            name,
            ELECTRS_ORIGIN.to_string(),
            OracleInfo {
                endpoint: ORACLE_ORIGIN.to_string(),
                public_key: XOnlyPublicKey::from_str(ORACLE_PUBKEY)?,
            },
            storage,
            settings,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn start_test(
        name: &str,
        electrs_origin: String,
        oracle: OracleInfo,
        node_storage: Arc<InMemoryStore>,
        settings: XXINodeSettings,
    ) -> Result<(Arc<Self>, RunningNode)> {
        let data_dir = random_tmp_dir().join(name);

        let seed = Bip39Seed::new().expect("A valid bip39 seed");

        let mut ephemeral_randomness = [0; 32];
        thread_rng().fill_bytes(&mut ephemeral_randomness);

        let address = {
            let listener = TcpListener::bind("0.0.0.0:0").unwrap();
            listener.local_addr().expect("To get a free local address")
        };

        let storage = TenTenOneInMemoryStorage::new();
        let wallet_storage = on_chain_wallet::InMemoryStorage::new();

        let (dlc_event_sender, dlc_event_receiver) = mpsc::channel::<DlcChannelEvent>();
        let event_handler = Arc::new(NodeEventHandler::new());
        let node = Node::new(
            name,
            Network::Regtest,
            data_dir.as_path(),
            storage,
            node_storage,
            wallet_storage,
            address,
            address,
            electrs_origin,
            seed,
            ephemeral_randomness,
            settings,
            vec![oracle.into()],
            XOnlyPublicKey::from_str(ORACLE_PUBKEY)?,
            event_handler.clone(),
            dlc_event_sender,
        )?;
        let node = Arc::new(node);

        tokio::spawn({
            let mut receiver = event_handler.subscribe();
            let node = node.clone();
            async move {
                let mut queue = HashMap::new();
                loop {
                    match receiver.recv().await {
                        Ok(NodeEvent::StoreDlcMessage { peer, msg }) => {
                            queue.insert(peer, msg);
                        }
                        Ok(NodeEvent::SendLastDlcMessage { peer }) => match queue.get(&peer) {
                            Some(msg) => {
                                send_dlc_message(
                                    &node.dlc_message_handler,
                                    &node.peer_manager,
                                    peer,
                                    msg.clone(),
                                );
                            }
                            None => {
                                tracing::warn!(%peer, "No last dlc message found in queue.");
                            }
                        },
                        Ok(NodeEvent::SendDlcMessage { peer, msg }) => {
                            send_dlc_message(
                                &node.dlc_message_handler,
                                &node.peer_manager,
                                peer,
                                msg,
                            );
                        }
                        Ok(NodeEvent::Connected { .. }) => {} // ignored
                        Ok(NodeEvent::DlcChannelEvent { .. }) => {} // ignored
                        Err(_) => {
                            tracing::error!(
                                "Failed to receive message from node event handler channel."
                            );
                            break;
                        }
                    }
                }
            }
        });

        let running = node.start(dlc_event_receiver)?;

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
        self.sync_on_chain_wallet().await
    }

    async fn fund(&self, amount: Amount, n_utxos: u64) -> Result<()> {
        let starting_balance = self.get_confirmed_balance();
        let expected_balance = starting_balance + amount.to_sat();

        // we mine blocks so that the internal wallet in bitcoind has enough utxos to fund the
        // wallet
        bitcoind::mine(n_utxos as u16 + 1).await?;
        for _ in 0..n_utxos {
            let address = self.wallet.get_new_address().unwrap();
            bitcoind::fund(
                address.to_string(),
                Amount::from_sat(amount.to_sat() / n_utxos),
            )
            .await?;
        }
        bitcoind::mine(1).await?;

        tokio::time::timeout(Duration::from_secs(30), async {
            while self.get_confirmed_balance() < expected_balance {
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

    fn get_confirmed_balance(&self) -> u64 {
        self.get_on_chain_balance().confirmed
    }

    pub fn disconnect(&self, peer: NodeInfo) {
        self.peer_manager
            .disconnect_by_node_id(to_secp_pk_29(peer.pubkey))
    }

    pub async fn reconnect(&self, peer: NodeInfo) -> Result<()> {
        self.disconnect(peer);
        tokio::time::sleep(Duration::from_secs(1)).await;
        self.connect_once(peer).await?;
        Ok(())
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

fn xxi_node_settings_coordinator() -> XXINodeSettings {
    XXINodeSettings {
        off_chain_sync_interval: Duration::from_secs(5),
        on_chain_sync_interval: Duration::from_secs(300),
        fee_rate_sync_interval: Duration::from_secs(20),
        sub_channel_manager_periodic_check_interval: Duration::from_secs(30),
        shadow_sync_interval: Duration::from_secs(600),
    }
}

fn xxi_node_settings_app() -> XXINodeSettings {
    XXINodeSettings {
        off_chain_sync_interval: Duration::from_secs(5),
        on_chain_sync_interval: Duration::from_secs(300),
        fee_rate_sync_interval: Duration::from_secs(20),
        sub_channel_manager_periodic_check_interval: Duration::from_secs(30),
        shadow_sync_interval: Duration::from_secs(600),
    }
}

fn dummy_contract_input(
    offer_collateral: u64,
    accept_collateral: u64,
    oracle_pk: XOnlyPublicKey,
    fee_rate_sats_per_vbyte: Option<u64>,
) -> ContractInput {
    let total_collateral = offer_collateral + accept_collateral;

    let n_cets = 100;
    let rounding_mod = total_collateral / (n_cets + 1);

    let maturity_time = OffsetDateTime::now_utc() + time::Duration::days(7);
    let maturity_time = maturity_time.unix_timestamp() as u64;

    ContractInput {
        offer_collateral,
        accept_collateral,
        fee_rate: fee_rate_sats_per_vbyte.unwrap_or(2),
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
                    intervals: vec![
                        RoundingInterval {
                            begin_interval: 0,
                            rounding_mod: 1,
                        },
                        RoundingInterval {
                            begin_interval: 50_000,
                            rounding_mod,
                        },
                        RoundingInterval {
                            begin_interval: 60_000,
                            rounding_mod: 1,
                        },
                    ],
                },
                difference_params: None,
                oracle_numeric_infos: dlc_trie::OracleNumericInfo {
                    base: 2,
                    nb_digits: vec![20],
                },
            }),
            oracles: OracleInput {
                public_keys: vec![to_xonly_pk_29(oracle_pk)],
                event_id: format!("btcusd{maturity_time}"),
                threshold: 1,
            },
        }],
    }
}

pub fn new_reference_id() -> ReferenceId {
    let uuid = Uuid::new_v4();
    let hex = uuid.as_simple().to_hex();
    let bytes = hex.as_bytes();

    debug_assert!(bytes.len() == 32, "length must be exactly 32 bytes");

    let mut array = [0u8; 32];
    array.copy_from_slice(bytes);

    array
}
