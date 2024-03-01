use crate::config::ConfigInternal;
use bitcoin::key::XOnlyPublicKey;
use bitcoin::Network;
use flutter_rust_bridge::frb;
use std::str::FromStr;

#[frb]
#[derive(Debug, Clone)]
pub struct Config {
    pub coordinator_pubkey: String,
    pub electrs_endpoint: String,
    pub host: String,
    pub p2p_port: u16,
    pub http_port: u16,
    pub network: String,
    pub oracle_endpoint: String,
    pub oracle_pubkey: String,
    pub health_check_interval_secs: u64,
    pub rgs_server_url: Option<String>,
}

pub struct Directories {
    pub app_dir: String,
    pub seed_dir: String,
}

impl From<(Config, Directories)> for ConfigInternal {
    fn from(value: (Config, Directories)) -> Self {
        let config = value.0;
        let dirs = value.1;

        tracing::debug!(?config, "Parsing config from flutter");

        // Make sure that the `RGS_SERVER_URL` environment variable is not an empty string.
        let rgs_server_url = {
            match config.rgs_server_url {
                Some(rgs_server_url) if rgs_server_url.is_empty() => None,
                Some(rgs_server_url) => Some(rgs_server_url),
                None => None,
            }
        };

        Self {
            coordinator_pubkey: config.coordinator_pubkey.parse().expect("PK to be valid"),
            electrs_endpoint: config.electrs_endpoint,
            http_endpoint: format!("{}:{}", config.host, config.http_port)
                .parse()
                .expect("host and http_port to be valid"),
            p2p_endpoint: format!("{}:{}", config.host, config.p2p_port)
                .parse()
                .expect("host and p2p_port to be valid"),
            network: parse_network(&config.network),
            oracle_endpoint: config.oracle_endpoint,
            oracle_pubkey: XOnlyPublicKey::from_str(config.oracle_pubkey.as_str())
                .expect("Valid oracle public key"),
            health_check_interval: std::time::Duration::from_secs(
                config.health_check_interval_secs,
            ),
            data_dir: dirs.app_dir,
            seed_dir: dirs.seed_dir,
            rgs_server_url,
        }
    }
}

pub fn parse_network(network: &str) -> Network {
    match network {
        "signet" => Network::Signet,
        "testnet" => Network::Testnet,
        "mainnet" => Network::Bitcoin,
        "bitcoin" => Network::Bitcoin,
        _ => Network::Regtest,
    }
}
