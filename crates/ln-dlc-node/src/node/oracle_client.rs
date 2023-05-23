use crate::node::Node;
use bitcoin::secp256k1::XOnlyPublicKey;
use dlc_manager::Oracle;
use p2pd_oracle_client::P2PDOracleClient;
use std::str::FromStr;

// TODO: This should come from the configuration.
const ORACLE_ENDPOINT: &str = "https://oracle.holzeis.me/";

pub fn build() -> P2PDOracleClient {
    P2PDOracleClient {
        host: ORACLE_ENDPOINT.to_string(),
        public_key: XOnlyPublicKey::from_str(
            "16f88cf7d21e6c0f46bcbc983a4e3b19726c6c98858cc31c83551a88fde171c0",
        )
        .expect("To be a valid pubkey"),
    }
}

impl<P> Node<P> {
    pub fn oracle_pk(&self) -> XOnlyPublicKey {
        self.oracle.get_public_key()
    }
}
