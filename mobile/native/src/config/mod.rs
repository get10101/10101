use bitcoin::key::XOnlyPublicKey;
use bitcoin::secp256k1::PublicKey;
use bitcoin::Network;
use std::net::SocketAddr;
use std::path::Path;
use std::time::Duration;
use xxi_node::node::NodeInfo;
use xxi_node::node::OracleInfo;

pub mod api;

#[derive(Clone)]
pub struct ConfigInternal {
    coordinator_pubkey: PublicKey,
    electrs_endpoint: String,
    http_endpoint: SocketAddr,
    #[allow(dead_code)] // Irrelevant when using websockets
    p2p_endpoint: SocketAddr,
    network: Network,
    oracle_endpoint: String,
    oracle_pubkey: XOnlyPublicKey,
    health_check_interval: Duration,
    data_dir: String,
    seed_dir: String,
}

pub fn coordinator_health_endpoint() -> String {
    let config = crate::state::get_config();
    format!("http://{}/health", config.http_endpoint)
}

pub fn health_check_interval() -> Duration {
    crate::state::get_config().health_check_interval
}

pub fn get_coordinator_info() -> NodeInfo {
    let config = crate::state::get_config();

    #[cfg(feature = "ws")]
    #[allow(unused_variables)] // In case both features are enabled
    let (address, is_ws) = (config.http_endpoint, true);

    #[cfg(feature = "native_tcp")]
    let (address, is_ws) = (config.p2p_endpoint, false);

    NodeInfo {
        pubkey: config.coordinator_pubkey,
        address,
        is_ws,
    }
}

pub fn get_electrs_endpoint() -> String {
    crate::state::get_config().electrs_endpoint
}

pub fn get_oracle_info() -> OracleInfo {
    let config = crate::state::get_config();
    OracleInfo {
        endpoint: config.oracle_endpoint.clone(),
        public_key: config.oracle_pubkey,
    }
}

pub fn get_http_endpoint() -> SocketAddr {
    crate::state::get_config().http_endpoint
}

pub fn get_network() -> Network {
    crate::state::get_config().network
}

pub fn get_data_dir() -> String {
    crate::state::get_config().data_dir
}

pub fn get_seed_dir() -> String {
    crate::state::get_config().seed_dir
}

pub fn get_backup_dir() -> String {
    Path::new(&get_data_dir())
        .join(get_network().to_string())
        .join("backup")
        .to_string_lossy()
        .to_string()
}
