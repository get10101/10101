use crate::ln::app_config;
use crate::ln::coordinator_config;
use crate::node::LnDlcNodeSettings;
use crate::node::Node;
use crate::node::NodeInfo;
use crate::node::PaymentMap;
use crate::seed::Bip39Seed;
use crate::util;
use anyhow::Result;
use bitcoin::Address;
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
use futures::Future;
use lightning::ln::channelmanager::ChannelDetails;
use lightning::util::config::UserConfig;
use rand::distributions::Alphanumeric;
use rand::thread_rng;
use rand::Rng;
use rand::RngCore;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use std::env::temp_dir;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Once;
use std::time::Duration;

mod bitcoind;
mod dlc;
mod just_in_time_channel;
mod lnd;
mod multi_hop_payment;
mod onboard_from_lnd;
mod single_hop_payment;

#[cfg(feature = "load_tests")]
mod load;

const ESPLORA_ORIGIN: &str = "http://localhost:3000";
const FAUCET_ORIGIN: &str = "http://localhost:8080";

fn init_tracing() {
    static TRACING_TEST_SUBSCRIBER: Once = Once::new();

    TRACING_TEST_SUBSCRIBER.call_once(|| {
        tracing_subscriber::fmt()
            .with_env_filter(
                "debug,hyper=warn,reqwest=warn,rustls=warn,bdk=info,lightning=trace,sled=info,lightning::chain::channelmonitor=trace,lightning::ln::peer_handler=debug,ureq=info",
            )
            .with_test_writer()
            .init()
    })
}

impl Node<PaymentMap> {
    fn start_test_app(name: &str) -> Result<Self> {
        Self::start_test(name, app_config(), ESPLORA_ORIGIN.to_string())
    }

    fn start_test_coordinator(name: &str) -> Result<Self> {
        Self::start_test(name, coordinator_config(), ESPLORA_ORIGIN.to_string())
    }

    fn start_test(name: &str, user_config: UserConfig, esplora_origin: String) -> Result<Self> {
        let data_dir = random_tmp_dir().join(name);

        let seed = Bip39Seed::new().expect("A valid bip39 seed");

        let mut ephemeral_randomness = [0; 32];
        thread_rng().fill_bytes(&mut ephemeral_randomness);

        let address = {
            let listener = TcpListener::bind("0.0.0.0:0").unwrap();
            listener.local_addr().expect("To get a free local address")
        };

        let node = Node::new(
            name,
            Network::Regtest,
            data_dir.as_path(),
            PaymentMap::default(),
            address,
            address,
            vec![util::build_net_address(address.ip(), address.port())],
            esplora_origin,
            seed,
            ephemeral_randomness,
            user_config,
            LnDlcNodeSettings::default(),
        )?;

        tracing::debug!(%name, info = %node.info, "Node started");

        Ok(node)
    }

    async fn fund(&self, amount: Amount) -> Result<()> {
        let starting_balance = self.get_confirmed_balance().await?;
        let expected_balance = starting_balance + amount.to_sat();

        let address = self.wallet.get_last_unused_address()?;

        fund_and_mine(address, amount).await?;

        while self.get_confirmed_balance().await? < expected_balance {
            let interval = Duration::from_millis(200);

            self.wallet().sync().await.unwrap();

            tokio::time::sleep(interval).await;
            tracing::debug!(
                ?interval,
                "Checking if wallet has been funded after interval"
            )
        }

        Ok(())
    }

    async fn get_confirmed_balance(&self) -> Result<u64> {
        let balance = self.wallet.inner().get_balance()?;

        Ok(balance.confirmed)
    }

    /// Initiates the opening of a channel _and_ waits for the channel
    /// to be usable.
    async fn open_channel(
        &self,
        peer: &Node<PaymentMap>,
        amount_us: u64,
        amount_them: u64,
    ) -> Result<ChannelDetails> {
        let temp_channel_id = self.initiate_open_channel(
            peer.info.pubkey,
            amount_us + amount_them,
            amount_them,
            false,
        )?;

        // The config flag
        // `user_config.manually_accept_inbound_channels` implies that
        // the peer will accept 0-conf channels
        if !peer.user_config.manually_accept_inbound_channels {
            let required_confirmations = peer.user_config.channel_handshake_config.minimum_depth;

            bitcoind::mine(required_confirmations as u16).await?;
        }

        let channel_details = tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                if let Some(details) = self
                    .channel_manager
                    .list_usable_channels()
                    .iter()
                    .find(|c| c.counterparty.node_id == peer.info.pubkey)
                {
                    break details.clone();
                }

                // Only sync if 0-conf channels are disabled
                if !peer.user_config.manually_accept_inbound_channels {
                    // We need to sync both parties, even if
                    // `trust_own_funding_0conf` is true for the creator
                    // of the channel (`self`)
                    self.wallet().sync().await.unwrap();
                    peer.wallet().sync().await.unwrap();
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

async fn fund_and_mine(address: Address, amount: Amount) -> Result<()> {
    bitcoind::fund(address.to_string(), amount).await?;
    bitcoind::mine(1).await?;
    Ok(())
}

/// Calculate the "minimum" acceptable value for the outbound liquidity
/// of the channel creator.
///
/// The value calculated is not guaranteed to be the exact minimum,
/// but it should be close enough.
///
/// This is useful when the channel creator wants to push as many
/// coins as possible to their peer on channel creation.
fn min_outbound_liquidity_channel_creator(peer: &Node<PaymentMap>, peer_balance: u64) -> u64 {
    let min_reserve_millionths_creator = Decimal::from(
        peer.user_config
            .channel_handshake_config
            .their_channel_reserve_proportional_millionths,
    );

    let min_reserve_percent_creator = min_reserve_millionths_creator / Decimal::from(1_000_000);

    // This is an approximation as we assume that `channel_balance ~=
    // peer_balance`
    let channel_balance_estimate = Decimal::from(peer_balance);

    let min_reserve_creator = min_reserve_percent_creator * channel_balance_estimate;
    let min_reserve_creator = min_reserve_creator.to_u64().unwrap();

    // The minimum reserve for any party is actually hard-coded to
    // 1_000 sats by LDK
    let min_reserve_creator = min_reserve_creator.max(1_000);

    // This is just an upper bound
    let commit_transaction_fee = 1_000;

    min_reserve_creator + commit_transaction_fee
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
fn log_channel_id(node: &Node<PaymentMap>, index: usize, pair: &str) {
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
