use crate::node::Node;
use bitcoin::secp256k1::XOnlyPublicKey;
use dlc_manager::Oracle;
use p2pd_oracle_client::P2PDOracleClient;
use std::str::FromStr;

pub fn build(oracle_endpoint: String) -> P2PDOracleClient {
    // TODO: fetch public key from oracle at ORACLE_ENDPOINT/oracle/publickey

    P2PDOracleClient {
        host: oracle_endpoint + "/",
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
