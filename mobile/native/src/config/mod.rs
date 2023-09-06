pub mod api;

use crate::config::api::Config;
use bdk::bitcoin;
use bdk::bitcoin::secp256k1::PublicKey;
use bdk::bitcoin::XOnlyPublicKey;
use ln_dlc_node::node::NodeInfo;
use ln_dlc_node::node::OracleInfo;
use state::Storage;
use std::net::SocketAddr;
use std::time::Duration;

static CONFIG: Storage<ConfigInternal> = Storage::new();

#[derive(Clone)]
pub struct ConfigInternal {
    coordinator_pubkey: PublicKey,
    esplora_endpoint: String,
    http_endpoint: SocketAddr,
    p2p_endpoint: SocketAddr,
    network: bitcoin::Network,
    oracle_endpoint: String,
    oracle_pubkey: XOnlyPublicKey,
    health_check_interval: Duration,
}

impl ConfigInternal {
    pub fn coordinator_health_endpoint(&self) -> String {
        format!("http://{}/health", self.http_endpoint)
    }

    pub fn health_check_interval(&self) -> Duration {
        self.health_check_interval
    }
}

pub fn set(config: Config) {
    CONFIG.set(config.into());
}

pub fn get_coordinator_info() -> NodeInfo {
    let config = CONFIG.get();
    NodeInfo {
        pubkey: config.coordinator_pubkey,
        address: config.p2p_endpoint,
    }
}

pub fn get_esplora_endpoint() -> String {
    CONFIG.get().esplora_endpoint.clone()
}

pub fn get_oracle_info() -> OracleInfo {
    let config = CONFIG.get();
    OracleInfo {
        endpoint: config.oracle_endpoint.clone(),
        public_key: config.oracle_pubkey,
    }
}

pub fn get_http_endpoint() -> SocketAddr {
    CONFIG.get().http_endpoint
}

pub fn get_network() -> bitcoin::Network {
    CONFIG.get().network
}
