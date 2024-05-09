use bitcoin::secp256k1::PublicKey;
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportedError {
    pub trader_pk: PublicKey,
    pub msg: String,
    pub version: Option<String>,
}
