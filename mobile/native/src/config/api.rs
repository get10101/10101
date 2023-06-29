use crate::config::ConfigInternal;
use bdk::bitcoin::Network;
use bdk::bitcoin::XOnlyPublicKey;
use flutter_rust_bridge::frb;
use std::str::FromStr;
use url::Url;

#[frb]
#[derive(Debug)]
pub struct Config {
    pub coordinator_pubkey: String,
    pub esplora_endpoint: String,
    pub host: String,
    pub p2p_port: u16,
    pub http_port: u16,
    pub network: String,
    pub oracle_endpoint: String,
    pub oracle_pubkey: String,
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
