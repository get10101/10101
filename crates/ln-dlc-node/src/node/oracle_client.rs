use crate::node::Node;
use anyhow::Result;
use bitcoin::secp256k1::XOnlyPublicKey;
use dlc_manager::Oracle;
use p2pd_oracle_client::P2PDOracleClient;
use std::thread::sleep;
use std::time::Duration;

// TODO: This should come from the configuration.
const ORACLE_ENDPOINT: &str = "https://oracle.holzeis.me/";
const ORACLE_CONNECT_RETRY_INTERVAL: Duration = Duration::from_secs(1);
const ORACLE_CONNECT_RETRY_LIMIT: i32 = 10;

pub fn build() -> Result<P2PDOracleClient> {
    let mut count = 0;
    tracing::debug!("Building oracle client...");
    loop {
        count += 1;
        match P2PDOracleClient::new(ORACLE_ENDPOINT) {
            Ok(oracle) => return Ok(oracle),
            Err(e) if count == ORACLE_CONNECT_RETRY_LIMIT => {
                anyhow::bail!(
                    "Failed to build oracle client due to {e:#}, retry limit {} reached",
                    count
                );
            }
            Err(e) => tracing::debug!(
                "Retrying building oracle client..., attempts remaining {}: {e:#}",
                ORACLE_CONNECT_RETRY_LIMIT - count
            ),
        }
        sleep(ORACLE_CONNECT_RETRY_INTERVAL);
    }
}

impl<P> Node<P> {
    pub fn oracle_pk(&self) -> XOnlyPublicKey {
        self.oracle.get_public_key()
    }
}
