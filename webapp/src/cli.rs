use anyhow::ensure;
use anyhow::Result;
use clap::Parser;
use sha2::digest::FixedOutput;
use sha2::Digest;
use sha2::Sha256;
use std::env::current_dir;
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Parser)]
pub struct Opts {
    #[clap(
        long,
        default_value = "02dd6abec97f9a748bf76ad502b004ce05d1b2d1f43a9e76bd7d85e767ffb022c9@127.0.0.1:9045"
    )]
    pub coordinator_endpoint: String,

    #[clap(long, default_value = "8000")]
    pub coordinator_http_port: u16,

    /// Where to permanently store data, defaults to the current working directory.
    #[clap(long)]
    data_dir: Option<PathBuf>,

    #[clap(value_enum, default_value = "regtest")]
    pub network: Network,

    /// The address to connect to the Electrs API.
    #[clap(long, default_value = "http://localhost:3000", aliases = ["esplora"])]
    pub electrs: String,

    /// The endpoint of the p2p-derivatives oracle
    #[clap(
        long,
        default_value = "16f88cf7d21e6c0f46bcbc983a4e3b19726c6c98858cc31c83551a88fde171c0@http://127.0.0.1:8081"
    )]
    oracle: String,

    /// Where to find the cert and key pem files
    #[clap(long)]
    cert_dir: Option<PathBuf>,

    #[clap(long, default_value = "satoshi")]
    password: String,

    #[clap(long)]
    pub secure: bool,

    #[clap(long)]
    pub whitelist_withdrawal_addresses: bool,

    /// The whitelisted bitcoin addresses the wallet should be allowed to send to. Only honoured if
    /// the [`whitelist_withdrawal_addresses`] flag is set to true.
    #[arg(num_args(0..))]
    #[clap(long)]
    pub withdrawal_address: Vec<String>,
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

    pub fn password(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.password.as_bytes());
        hex::encode(hasher.finalize_fixed())
    }

    pub fn data_dir(&self) -> Result<PathBuf> {
        let data_dir = match self.data_dir.clone() {
            None => current_dir()?.join("data"),
            Some(path) => path,
        }
        .join("webapp");

        Ok(data_dir)
    }

    pub fn cert_dir(&self) -> Result<PathBuf> {
        let cert_dir = match self.cert_dir.clone() {
            None => current_dir()?.join("webapp/certs"),
            Some(path) => path,
        };

        Ok(cert_dir)
    }

    pub fn coordinator_pubkey(&self) -> Result<String> {
        let coordinator: Vec<&str> = self.coordinator_endpoint.split('@').collect();
        ensure!(coordinator.len() == 2, "invalid coordinator endpoint");

        Ok(coordinator
            .first()
            .expect("valid coordinator endpoint")
            .to_string())
    }

    pub fn coordinator_endpoint(&self) -> Result<String> {
        let coordinator: Vec<&str> = self.coordinator_endpoint.split('@').collect();
        ensure!(coordinator.len() == 2, "invalid coordinator endpoint");

        let coordinator = coordinator
            .get(1)
            .expect("valid coordinator endpoint")
            .to_string();

        let coordinator: Vec<&str> = coordinator.split(':').collect();

        ensure!(coordinator.len() == 2, "invalid coordinator endpoint");

        Ok(coordinator
            .first()
            .expect("valid coordinator endpoint")
            .to_string())
    }

    pub fn coordinator_p2p_port(&self) -> Result<u16> {
        let coordinator: Vec<&str> = self.coordinator_endpoint.split('@').collect();
        ensure!(coordinator.len() == 2, "invalid coordinator endpoint");

        let coordinator = coordinator
            .get(1)
            .expect("valid coordinator endpoint")
            .to_string();

        let coordinator: Vec<&str> = coordinator.split(':').collect();

        ensure!(coordinator.len() == 2, "invalid coordinator endpoint");

        Ok(
            u16::from_str(coordinator.get(1).expect("valid coordinator endpoint"))
                .expect("valid coordinator endpoint"),
        )
    }

    pub fn oracle_pubkey(&self) -> Result<String> {
        let oracle: Vec<&str> = self.oracle.split('@').collect();
        ensure!(oracle.len() == 2, "invalid oracle endpoint");

        Ok(oracle.first().expect("valid oracle endpoint").to_string())
    }

    pub fn oracle_endpoint(&self) -> Result<String> {
        let oracle: Vec<&str> = self.oracle.split('@').collect();
        ensure!(oracle.len() == 2, "invalid oracle endpoint");

        Ok(oracle.get(1).expect("valid oracle endpoint").to_string())
    }
}
