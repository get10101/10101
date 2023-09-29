pub mod api;

use crate::config::api::Config;
use bdk::bitcoin;
use bdk::bitcoin::secp256k1::PublicKey;
use bdk::bitcoin::XOnlyPublicKey;
use lightning::ln::channelmanager::MIN_CLTV_EXPIRY_DELTA;
use lightning::util::config::ChannelConfig;
use lightning::util::config::ChannelHandshakeConfig;
use lightning::util::config::ChannelHandshakeLimits;
use lightning::util::config::UserConfig;
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

pub fn app_config() -> UserConfig {
    UserConfig {
        channel_handshake_config: ChannelHandshakeConfig {
            // The app will only accept private channels. As we are forcing the apps announced
            // channel preferences, the coordinator needs to override this config to match the apps
            // preferences.
            announced_channel: false,
            minimum_depth: 1,
            // There is no risk in the leaf channel to receive 100% of the channel capacity.
            max_inbound_htlc_value_in_flight_percent_of_channel: 100,
            // We want the coordinator to recover force-close funds as soon as possible. We choose
            // 144 because we can't go any lower according to LDK.
            our_to_self_delay: 144,
            ..Default::default()
        },
        channel_handshake_limits: ChannelHandshakeLimits {
            max_minimum_depth: 1,
            trust_own_funding_0conf: true,
            // Enforces that incoming channels will be private.
            force_announced_channel_preference: true,
            // We want app users to only have to wait ~24 hours in case of a force-close. We choose
            // 144 because we can't go any lower according to LDK.
            their_to_self_delay: 144,
            max_funding_satoshis: 100_000_000,
            ..Default::default()
        },
        channel_config: ChannelConfig {
            cltv_expiry_delta: MIN_CLTV_EXPIRY_DELTA,
            ..Default::default()
        },
        // we want to accept 0-conf channels from the coordinator
        manually_accept_inbound_channels: true,
        ..Default::default()
    }
}
