use crate::config::ConfigInternal;
use bdk::bitcoin::Network;
use flutter_rust_bridge::frb;

#[frb]
#[derive(Debug)]
pub struct Config {
    pub coordinator_pubkey: String,
    pub electrs_endpoint: String,
    pub host: String,
    pub p2p_port: u16,
    pub http_port: u16,
    pub network: String,
}

impl From<Config> for ConfigInternal {
    fn from(config: Config) -> Self {
        Self {
            coordinator_pubkey: config.coordinator_pubkey.parse().expect("PK to be valid"),
            electrs_endpoint: config
                .electrs_endpoint
                .parse()
                .expect("electrs endpoint to be valid"),
            http_endpoint: format!("{}:{}", config.host, config.http_port)
                .parse()
                .expect("host and http_port to be valid"),
            p2p_endpoint: format!("{}:{}", config.host, config.p2p_port)
                .parse()
                .expect("host and p2p_port to be valid"),
            network: parse_network(&config.network),
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
