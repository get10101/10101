use anyhow::Result;
use clap::Parser;
use ln_dlc_node::node::OracleInfo;
use reqwest::Url;
use std::env::current_dir;
use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Parser)]
pub struct Opts {
    /// The address to listen on for the Lightning and `rust-dlc` p2p API.
    #[clap(long, default_value = "0.0.0.0:19045")]
    pub p2p_address: SocketAddr,

    /// Our own HTTP endpoint.
    #[clap(long, default_value = "0.0.0.0:18000")]
    pub http_address: SocketAddr,

    /// Where to permanently store data. Defaults to the current working directory.
    #[clap(long)]
    data_dir: Option<PathBuf>,

    #[clap(value_enum, default_value = "regtest")]
    pub network: Network,

    /// The orderbook HTTP endpoint.
    #[clap(long, default_value = "http://localhost:8000")]
    pub orderbook: Url,

    /// The address where to find the database including username and password.
    #[clap(
        long,
        default_value = "postgres://postgres:mysecretpassword@localhost:5432/orderbook"
    )]
    pub database: String,

    /// The Esplora server endpoint.
    #[clap(long, default_value = "http://localhost:3000")]
    pub esplora: String,

    /// If enabled logs will be in JSON format.
    #[clap(short, long)]
    pub json: bool,

    /// Amount of concurrent orders (buy,sell) that the maker will create at a time.
    #[clap(long, default_value = "5")]
    pub concurrent_orders: usize,

    /// Orders created by maker will be valid for this number of seconds.
    #[clap(long, default_value = "60")]
    pub order_expiry_after_seconds: u64,

    /// The oracle endpoint.
    #[clap(long, default_value = "http://localhost:8081")]
    oracle_endpoint: String,

    /// The public key of the oracle.
    #[clap(
        long,
        default_value = "16f88cf7d21e6c0f46bcbc983a4e3b19726c6c98858cc31c83551a88fde171c0"
    )]
    oracle_pubkey: String,

    /// BitMEX API key.
    #[clap(long)]
    pub bitmex_api_key: Option<String>,

    /// BitMEX API secret.
    #[clap(long)]
    pub bitmex_api_secret: Option<String>,

    /// RGS server URL.
    #[clap(long)]
    pub rgs_server_url: Option<String>,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Network {
    Regtest,
    Signet,
    Testnet,
    Mainnet,
}

impl From<Network> for bitcoin::Network {
    fn from(network: Network) -> Self {
        match network {
            Network::Regtest => bitcoin::Network::Regtest,
            Network::Signet => bitcoin::Network::Signet,
            Network::Testnet => bitcoin::Network::Testnet,
            Network::Mainnet => bitcoin::Network::Bitcoin,
        }
    }
}

impl Opts {
    // use this method to parse the options from the cli.
    pub fn read() -> Opts {
        Opts::parse()
    }

    pub fn network(&self) -> bitcoin::Network {
        self.network.into()
    }

    pub fn data_dir(&self) -> Result<PathBuf> {
        let data_dir = match self.data_dir.clone() {
            None => current_dir()?.join("data"),
            Some(path) => path,
        }
        .join("maker");

        Ok(data_dir)
    }

    pub fn get_oracle_info(&self) -> OracleInfo {
        OracleInfo {
            endpoint: self.oracle_endpoint.clone(),
            public_key: self
                .oracle_pubkey
                .as_str()
                .parse()
                .expect("Valid oracle public key"),
        }
    }
}
