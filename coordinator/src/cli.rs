use anyhow::Result;
use bitcoin::secp256k1::XOnlyPublicKey;
use clap::Parser;
use std::env::current_dir;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;
use xxi_node::node::OracleInfo;

#[derive(Parser)]
pub struct Opts {
    /// The address to listen on for the lightning and dlc peer2peer API.
    #[clap(long, default_value = "0.0.0.0:9045")]
    pub p2p_address: SocketAddr,

    /// The IP address to listen on for the HTTP API.
    #[clap(long, default_value = "0.0.0.0:8000")]
    pub http_address: SocketAddr,

    /// Where to permanently store data, defaults to the current working directory.
    #[clap(long)]
    data_dir: Option<PathBuf>,

    #[clap(value_enum, default_value = "regtest")]
    pub network: Network,

    /// If enabled logs will be in json format
    #[clap(short, long)]
    pub json: bool,

    /// The address where to find the database including username and password
    #[clap(
        long,
        default_value = "postgres://postgres:mysecretpassword@localhost:5432/orderbook"
    )]
    pub database: String,

    /// The address to connect to the Electrs API.
    #[clap(long, default_value = "http://localhost:3000", aliases = ["esplora"])]
    pub electrs: String,

    /// If enabled, tokio runtime can be locally debugged with tokio_console
    #[clap(long)]
    pub tokio_console: bool,

    /// If specified, metrics will be printed at the given interval
    #[clap(long)]
    pub tokio_metrics_interval_seconds: Option<u64>,

    /// Server API key for the LSP notification service.
    /// If not specified, the notifications will not be sent.
    #[clap(long, default_value = "")]
    pub fcm_api_key: String,

    /// The endpoint of the p2p-derivatives oracle
    #[arg(num_args(0..))]
    #[clap(
        long,
        default_value = "16f88cf7d21e6c0f46bcbc983a4e3b19726c6c98858cc31c83551a88fde171c0@http://localhost:8081"
    )]
    oracle: Vec<String>,

    /// Defines the default oracle to be used for propose a dlc channel. Note this pubkey has to be
    /// included in the oracle arguments.
    ///
    /// FIXME(holzeis): Remove this argument once we have migrated successfully from the old oracle
    /// to the new one. This is needed to instruct the coordinator to use only the new oracle for
    /// proposing dlc channels.
    #[clap(
        long,
        default_value = "16f88cf7d21e6c0f46bcbc983a4e3b19726c6c98858cc31c83551a88fde171c0"
    )]
    pub oracle_pubkey: String,

    /// The endpoint of the lnd rest api
    #[clap(long, default_value = "localhost:18080")]
    pub lnd_endpoint: String,

    /// Defines the macaroon to be used for the lnd http api.
    #[clap(long, default_value = "")]
    pub macaroon: String,

    /// If enabled the coordinator will try to connect to lnd via https, wss.
    #[clap(short, long)]
    pub secure_lnd: bool,
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

    pub fn get_oracle_infos(&self) -> Vec<OracleInfo> {
        self.oracle
            .iter()
            .map(|oracle| {
                let oracle: Vec<&str> = oracle.split('@').collect();
                OracleInfo {
                    public_key: XOnlyPublicKey::from_str(
                        oracle.first().expect("public key to be set"),
                    )
                    .expect("Valid oracle public key"),
                    endpoint: oracle.get(1).expect("endpoint to be set").to_string(),
                }
            })
            .collect()
    }

    pub fn data_dir(&self) -> Result<PathBuf> {
        let data_dir = match self.data_dir.clone() {
            None => current_dir()?.join("data"),
            Some(path) => path,
        }
        .join("coordinator");

        Ok(data_dir)
    }
}
