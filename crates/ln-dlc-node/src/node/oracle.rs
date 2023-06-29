use crate::node::Node;
use bitcoin::secp256k1::XOnlyPublicKey;
use dlc_manager::Oracle;
use p2pd_oracle_client::P2PDOracleClient;
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OracleInfo {
    pub endpoint: String,
    pub public_key: XOnlyPublicKey,
}

impl From<OracleInfo> for P2PDOracleClient {
    fn from(oracle: OracleInfo) -> Self {
        P2PDOracleClient {
            host: oracle.endpoint + "/",
            public_key: oracle.public_key,
        }
    }
}

impl<P> Node<P> {
    pub fn oracle_pk(&self) -> XOnlyPublicKey {
        self.oracle.get_public_key()
    }
}
