use anyhow::Result;
use clap::Parser;
use reqwest::Url;
use std::env::current_dir;
use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Parser)]
pub struct Opts {
    /// The address to listen on for the lightning and dlc peer2peer API.
    #[clap(long, default_value = "0.0.0.0:19045")]
    pub p2p_address: SocketAddr,

    /// The IP address to listen on for the HTTP API.
    #[clap(long, default_value = "0.0.0.0:18000")]
    pub http_address: SocketAddr,

    /// Where to permanently store data, defaults to the current working directory.
    #[clap(long)]
    data_dir: Option<PathBuf>,

    #[clap(value_enum, default_value = "regtest")]
    pub network: Network,

    /// The HTTP address for the orderbook.
    #[clap(long, default_value = "http://localhost:8000")]
    pub orderbook: Url,

    /// The address where to find the database inclding username and password
    #[clap(
        long,
        default_value = "postgres://postgres:mysecretpassword@localhost:5432/orderbook"
    )]
    pub database: String,

    /// The address to connect esplora API to
    #[clap(long, default_value = "http://localhost:3000")]
    pub esplora: String,

    /// If enabled logs will be in json format
    #[clap(short, long)]
    pub json: bool,

    /// If enabled logs will be in json format
    #[clap(long, default_value = "5")]
    pub concurrent_orders: usize,
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
}
