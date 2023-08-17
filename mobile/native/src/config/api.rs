use crate::config::ConfigInternal;
use bdk::bitcoin::Network;
use bdk::bitcoin::XOnlyPublicKey;
use flutter_rust_bridge::frb;
use std::str::FromStr;
use url::Url;

#[frb]
#[derive(Debug, Clone)]
pub struct Config {
    pub coordinator_pubkey: String,
    pub esplora_endpoint: String,
    // Coordinator host
    pub host: String,
    // Coordinator p2p port
    pub p2p_port: u16,
    // Coordinator http port
    pub http_port: u16,
    pub network: String,
    pub oracle_endpoint: String,
    pub oracle_pubkey: String,
    pub health_check_interval_secs: u64,
}

impl Default for Config {
    /// Default config for the app connects to the public regtest network
    fn default() -> Self {
        Self {
            coordinator_pubkey:
                "03507b924dae6595cfb78492489978127c5f1e3877848564de2015cd6d41375802".to_string(),
            esplora_endpoint: "http://35.189.57.114:3000".to_string(),
            host: "35.189.57.114".to_string(),
            p2p_port: 9045,
            http_port: 80,
            network: "regtest".to_string(),
            oracle_endpoint: "http://35.189.57.114:8081".to_string(),
            oracle_pubkey: "5d12d79f575b8d99523797c46441c0549eb0defb6195fe8a080000cbe3ab3859"
                .to_string(),
            health_check_interval_secs: 10,
        }
    }
}

impl From<Config> for ConfigInternal {
    fn from(config: Config) -> Self {
        tracing::debug!(?config, "Parsing config from flutter");
        Self {
            coordinator_pubkey: config.coordinator_pubkey.parse().expect("PK to be valid"),
            esplora_endpoint: Url::parse(config.esplora_endpoint.as_str())
                .expect("esplora endpoint to be valid"),
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
        }
    }
}

fn parse_network(network: &str) -> Network {
    match network {
        "signet" => Network::Signet,
        "testnet" => Network::Testnet,
        "mainnet" => Network::Bitcoin,
        _ => Network::Regtest,
    }
}

/// Regtest config for the app
pub fn regtest_config() -> Config {
    Config::default()
}

/// Mainnet config for the app
pub fn mainnet_config() -> Config {
    Config {
        coordinator_pubkey: "022ae8dbec1caa4dac93f07f2ebf5ad7a5dd08d375b79f11095e81b065c2155156"
            .to_string(),
        esplora_endpoint: "https://blockstream.info/api".to_string(),
        host: "46.17.98.29".to_string(),
        p2p_port: 9045,
        http_port: 8000,
        network: "mainnet".to_string(),
        oracle_endpoint: "https://oracle.holzeis.me".to_string(),
        oracle_pubkey: "16f88cf7d21e6c0f46bcbc983a4e3b19726c6c98858cc31c83551a88fde171c0"
            .to_string(),
        ..Default::default()
    }
}
