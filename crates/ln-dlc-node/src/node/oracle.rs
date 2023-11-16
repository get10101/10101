use crate::node::Node;
use crate::node::Storage;
use crate::storage::TenTenOneStorage;
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

impl<S: TenTenOneStorage, N: Storage> Node<S, N> {
    pub fn oracle_pk(&self) -> Vec<XOnlyPublicKey> {
        self.oracles
            .clone()
            .into_iter()
            .map(|oracle| oracle.get_public_key())
            .collect()
    }
}
