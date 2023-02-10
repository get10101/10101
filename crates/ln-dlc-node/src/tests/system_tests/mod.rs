use crate::node::Node;
use crate::seed::Bip39Seed;
use anyhow::anyhow;
use anyhow::Result;
use bitcoin::Network;
use dlc_manager::Oracle;
use dlc_manager::Wallet;
use rand::thread_rng;
use rand::RngCore;
use serde::Serialize;
use std::env::temp_dir;
use std::net::TcpListener;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Once;
use std::time::Duration;

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

struct MockOracle;

impl Oracle for MockOracle {
    fn get_public_key(&self) -> bitcoin::XOnlyPublicKey {
        bitcoin::XOnlyPublicKey::from_str(
            "18845781f631c48f1c9709e23092067d06837f30aa0cd0544ac887fe91ddd166",
        )
        .unwrap()
    }

    fn get_announcement(
        &self,
        _event_id: &str,
    ) -> Result<dlc_messages::oracle_msgs::OracleAnnouncement, dlc_manager::error::Error> {
        todo!()
    }

    fn get_attestation(
        &self,
        _event_id: &str,
    ) -> Result<dlc_messages::oracle_msgs::OracleAttestation, dlc_manager::error::Error> {
        todo!()
    }
}

impl Node {
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

pub fn create_tmp_dir(dir_name: &str) -> PathBuf {
    let test_dir = temp_dir();
    let test_dir = test_dir.join(dir_name);
    let test_dir_str = test_dir.to_str().expect("To be a valid path");
    // TODO: why can't we use tracing here?
    println!("Current test dir location {test_dir_str}");
    test_dir
}

pub(crate) async fn setup_ln_node(test_dir: &Path, node_name: &str, seed: Bip39Seed) -> Node {
    let data_dir = test_dir.join(node_name);

    let mut ephemeral_randomness = [0; 32];
    thread_rng().fill_bytes(&mut ephemeral_randomness);

    let address = {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().expect("To get a free local address")
    };

    // todo: the tests are executed in the crates/ln-dlc-node directory, hence the folder will
    // be created there. but the creation will fail if the .ldk-data/alice/on_chain has not been
    // created before.
    Node::new(
        node_name.to_string(),
        Network::Regtest,
        data_dir.as_path(),
        address,
        ELECTRS_ORIGIN.to_string(),
        seed,
        ephemeral_randomness,
    )
    .await
}
