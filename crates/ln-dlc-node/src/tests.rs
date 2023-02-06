use dlc_manager::Oracle;
use serde::Serialize;
use std::str::FromStr;

mod add_dlc;
mod channel_less_payment;
mod dlc_collaborative_settlement;
mod dlc_non_collaborative_settlement;
mod single_hop_payment;

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

#[derive(Debug, Clone, Serialize)]
struct Faucet {
    address: String,
    amount: f32,
}

// TODO: this could be better wrapped.
async fn fund_and_mine(faucet: &Faucet) {
    let client = reqwest::Client::new();
    // mines a block and spends the given amount from the coinbase transaction to the given address
    let result = client
        .post("http://localhost:3000/faucet")
        .json(faucet)
        .send()
        .await
        .unwrap();

    assert!(result.status().is_success());
}
