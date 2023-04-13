use crate::node::Node;
use anyhow::anyhow;
use anyhow::Result;
use bitcoin::secp256k1::XOnlyPublicKey;
use dlc_manager::Oracle;
use p2pd_oracle_client::P2PDOracleClient;

pub async fn build() -> Result<P2PDOracleClient> {
    tokio::task::spawn_blocking(|| {
        P2PDOracleClient::new("https://oracle.holzeis.me/") // TODO: this should come form the configuration.
            .expect("to be able to create the p2pd oracle")
    })
    .await
    .map_err(|e| anyhow!(e))
}

impl<P> Node<P> {
    pub fn oracle_pk(&self) -> XOnlyPublicKey {
        self.oracle.get_public_key()
    }
}
