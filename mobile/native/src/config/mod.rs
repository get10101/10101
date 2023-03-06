pub mod api;

use crate::config::api::Config;
use bdk::bitcoin;
use bdk::bitcoin::secp256k1::PublicKey;
use ln_dlc_node::node::NodeInfo;
use state::Storage;
use std::net::SocketAddr;

static CONFIG: Storage<ConfigInternal> = Storage::new();

#[derive(Clone, Copy)]
struct ConfigInternal {
    coordinator_pubkey: PublicKey,
    electrs_endpoint: SocketAddr,
    http_endpoint: SocketAddr,
    p2p_endpoint: SocketAddr,
    network: bitcoin::Network,
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

pub fn get_electrs_endpoint() -> SocketAddr {
    CONFIG.get().electrs_endpoint
}

pub fn get_http_endpoint() -> SocketAddr {
    CONFIG.get().http_endpoint
}

pub fn get_network() -> bitcoin::Network {
    CONFIG.get().network
}
