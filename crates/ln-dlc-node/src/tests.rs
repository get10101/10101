//! For the task of writing the first test, we're using the following repos:
//!
//! - `get10101/rust-lightning` as a dependency.
//! - `get10101/rust-dlc` as a dependency.
//! - `p2pderivatives/ldk-sample` as an example.
//! - `get10101/10101-poc` as an example.
//!
//! We can use `p2pderivatives/ldk-sample` and `get10101/10101-poc`
//! to figure out how to set up the LN-DLC node. Also, we use
//! `p2pderivatives/ldk-sample` to consider which `rust-dlc` APIs we
//! might want to use for the tests.

use dlc_manager::Oracle;
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
