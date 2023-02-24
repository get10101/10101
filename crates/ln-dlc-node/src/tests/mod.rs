use crate::node::app_config;
use crate::node::coordinator_config;
use crate::node::Node;
use crate::node::NodeInfo;
use crate::seed::Bip39Seed;
use anyhow::anyhow;
use anyhow::Result;
use bitcoin::Address;
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
use dlc_manager::Wallet;
use futures::Future;
use lightning::ln::channelmanager::ChannelDetails;
use lightning::util::config::UserConfig;
use rand::distributions::Alphanumeric;
use rand::thread_rng;
use rand::Rng;
use rand::RngCore;
use serde::Serialize;
use std::env::temp_dir;
use std::mem;
use std::net::TcpListener;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Once;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

mod add_dlc;
mod dlc_collaborative_settlement;
mod dlc_non_collaborative_settlement;
mod just_in_time_channel;
mod multi_hop_payment;
mod single_hop_payment;

const CHOPSTICKS_FAUCET_ORIGIN: &str = "http://localhost:3000";
const ELECTRS_ORIGIN: &str = "tcp://localhost:50000";

fn init_tracing() {
    static TRACING_TEST_SUBSCRIBER: Once = Once::new();

    TRACING_TEST_SUBSCRIBER.call_once(|| {
        tracing_subscriber::fmt()
            .with_env_filter(
                "debug,hyper=warn,reqwest=warn,rustls=warn,bdk=info,ldk=debug,sled=info",
            )
            .with_test_writer()
            .init()
    })
}

impl Node {
    async fn start_test_app(name: &str) -> Result<Self> {
        Self::start_test(name, app_config()).await
    }

    async fn start_test_coordinator(name: &str) -> Result<Self> {
        Self::start_test(name, coordinator_config()).await
    }

    async fn start_test(name: &str, user_config: UserConfig) -> Result<Self> {
        let data_dir = random_tmp_dir().join(name);

        let seed = Bip39Seed::new().expect("A valid bip39 seed");

        let mut ephemeral_randomness = [0; 32];
        thread_rng().fill_bytes(&mut ephemeral_randomness);

        let address = {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.local_addr().expect("To get a free local address")
        };

        let node = Node::new(
            name.to_string(),
            Network::Regtest,
            data_dir.as_path(),
            address,
            ELECTRS_ORIGIN.to_string(),
            seed,
            ephemeral_randomness,
            user_config,
        )
        .await;

        let bg_processor = node.start().await?;
        mem::forget(bg_processor); // to keep it running

        tracing::debug!(%name, info = ?node.info, "Node started");

        Ok(node)
    }

    async fn fund(&self, amount: bitcoin::Amount) -> Result<()> {
        let starting_balance = self.get_confirmed_balance()?;
        let expected_balance = starting_balance + amount.to_sat();

        let address = self
            .wallet
            .get_new_address()
            .map_err(|_| anyhow!("Failed to get new address"))?;

        fund_and_mine(address, amount).await;

        while self.get_confirmed_balance()? < expected_balance {
            let interval = Duration::from_millis(200);

            self.sync();

            tokio::time::sleep(interval).await;
            tracing::debug!(
                ?interval,
                "Checking if wallet has been funded after interval"
            )
        }

        Ok(())
    }

    fn get_confirmed_balance(&self) -> Result<u64> {
        let balance = self.wallet.inner().get_balance()?;

        Ok(balance.confirmed)
    }

    /// Initiates the opening of a channel _and_ waits for the channel
    /// to be usable.
    ///
    /// We are assuming that a channel will be usable with 0
    /// confirmations. This depends on the channel's `UserConfig`'s of
    /// the peers involved!! It may not always work.
    async fn open_channel(
        &self,
        peer: &NodeInfo,
        amount_us: u64,
        amount_them: u64,
    ) -> Result<ChannelDetails> {
        let temp_channel_id =
            self.initiate_open_channel(*peer, amount_us + amount_them, amount_them)?;

        // TODO: Mine as many blocks as needed (and sync the wallets)
        // for the channel to become usable. Currently this assumes
        // support for 0-conf channels
        let channel_details = tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if let Some(details) = self
                    .channel_manager
                    .list_usable_channels()
                    .iter()
                    .find(|c| c.counterparty.node_id == peer.pubkey)
                {
                    break details.clone();
                }

                tracing::debug!(
                    %peer,
                    temp_channel_id = %hex::encode(temp_channel_id),
                    "Waiting for channel to be usable"
                );
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        })
        .await?;

        Ok(channel_details)
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
        let mut user_config = self.user_config;
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

    async fn accept_dlc_channel(&self, channel_id: &[u8; 32]) -> Result<()> {
        self.initiate_accept_dlc_channel_offer(channel_id)?;

        Ok(())
    }
}

/// Instructs `nigiri-chopsticks` to mine a block and spend the given `amount` from the coinbase
/// transaction to the given `address`.
async fn fund_and_mine(address: bitcoin::Address, amount: bitcoin::Amount) {
    #[derive(Serialize)]
    struct Payload {
        address: bitcoin::Address,
        #[serde(with = "bdk::bitcoin::util::amount::serde::as_btc")]
        amount: bitcoin::Amount,
    }

    let client = reqwest::Client::new();

    let result = client
        .post(format!("{CHOPSTICKS_FAUCET_ORIGIN}/faucet"))
        .json(&Payload { address, amount })
        .send()
        .await
        .unwrap();

    assert!(result.status().is_success());
}

/// Instructs `nigiri-chopsticks` to mine a block.
#[allow(dead_code)]
async fn mine(n: u32) {
    let address =
        Address::from_str("bcrt1qylgu6ffkp3p0m8tw8kp4tt2dmdh755f4r5dq7s").expect("valid address");

    for _ in 0..n {
        fund_and_mine(address.clone(), bitcoin::Amount::from_sat(1000)).await;
    }

    // For the mined blocks to be picked up by the subsequent wallet
    // syncs
    tokio::time::sleep(Duration::from_secs(5)).await;
}

fn random_tmp_dir() -> PathBuf {
    let tmp = temp_dir();

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
fn log_channel_id(node: &Node, index: usize, pair: &str) {
    let details = node
        .channel_manager
        .list_channels()
        .get(index)
        .unwrap()
        .clone();

    let channel_id = hex::encode(details.channel_id);
    let short_channel_id = details.short_channel_id.unwrap();
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

    let maturity_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 86_400; // in a day's time

    ContractInput {
        offer_collateral,
        accept_collateral,
        maturity_time: maturity_time as u32,
        fee_rate: 2,
        contract_infos: vec![ContractInputInfo {
            contract_descriptor: ContractDescriptor::Numerical(NumericalDescriptor {
                payout_function: PayoutFunction {
                    payout_function_pieces: vec![
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
                    ],
                },
                rounding_intervals: RoundingIntervals {
                    intervals: vec![RoundingInterval {
                        begin_interval: 0,
                        rounding_mod: 1,
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
