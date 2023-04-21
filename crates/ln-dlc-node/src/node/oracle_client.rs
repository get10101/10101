use crate::node::Node;
use anyhow::Context;
use anyhow::Result;
use bitcoin::secp256k1::XOnlyPublicKey;
use dlc_manager::Oracle;
use p2pd_oracle_client::P2PDOracleClient;

pub async fn build() -> Result<P2PDOracleClient> {
    tokio::task::spawn_blocking(|| {
        // TODO: This should come from the configuration.
        P2PDOracleClient::new("https://oracle.holzeis.me/").context("Failed to build oracle client")
    })
    .await
    .context("Failed to spawn oracle client")?
}

impl<P> Node<P> {
    pub fn oracle_pk(&self) -> XOnlyPublicKey {
        self.oracle.get_public_key()
    }
}
