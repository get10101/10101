use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use bitcoin::Amount;
use clap::Parser;
use ln_dlc_node::node::NodeInfo;
use local_ip_address::local_ip;
use reqwest::Response;
use reqwest::StatusCode;
use serde::Deserialize;
use std::time::Duration;
use tests_e2e::coordinator::Coordinator;
use tests_e2e::http::init_reqwest;
use tests_e2e::maker::Maker;
use tracing::metadata::LevelFilter;
use tracing_subscriber::filter::Directive;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

const RUST_LOG_ENV: &str = "RUST_LOG";

#[derive(Parser)]
pub struct Opts {
    /// Faucet address
    #[clap(long, default_value = "http://localhost:8080")]
    pub faucet: String,

    /// Bitcoind http url e.g. http://localhost:18443
    #[clap(long, default_value = "http://localhost:18443")]
    pub bitcoin: String,

    /// Bitcoind rpc username
    #[clap(long, default_value = "admin1")]
    pub bitcoin_user: String,

    /// Bitcoind rpc password
    #[clap(long, default_value = "123")]
    pub bitcoin_password: String,

    /// Coordinator address
    #[clap(long, default_value = "http://localhost:8000")]
    pub coordinator: String,

    /// Maker address
    #[clap(long, default_value = "http://localhost:18000")]
    pub maker: String,
}

#[tokio::main]
async fn main() {
    init_tracing(LevelFilter::DEBUG).expect("tracing to initialise");
    let opts = Opts::parse();
    ensure_bitcoin_is_usable(opts.bitcoin, opts.bitcoin_user, opts.bitcoin_password)
        .await
        .expect("Bitcoin might not be usable");
    fund_everything(&opts.faucet, &opts.coordinator, &opts.maker)
        .await
        .expect("to be able to fund");
}

async fn ensure_bitcoin_is_usable(
    bitcoin_url: String,
    username: String,
    password: String,
) -> Result<()> {
    use bitcoincore_rpc::Auth;
    use bitcoincore_rpc::Client;
    use bitcoincore_rpc::RpcApi;

    let rpc = Client::new(bitcoin_url.as_str(), Auth::UserPass(username, password))
        .expect("To be able to get bitcoin rpc client");
    let vec = rpc.list_wallets().expect("To be able to list wallets");
    if vec.is_empty() {
        tracing::info!("No wallet found in Bitcoind, creating one");
        rpc.create_wallet("regtest", None, None, None, None)
            .expect("To be able to create a wallet");
    } else {
        tracing::info!("Bitcoind wallet exists.");

        if let Err(err) = rpc.get_wallet_info() {
            tracing::info!("Wallet not loaded in Bitcoind, loading it... {err:#}");

            rpc.load_wallet("regtest")
                .expect("To be able to load wallet");
        }
    }

    let i = rpc.get_block_count()?;
    tracing::info!("Bitcoin has {i} blocks");
    if i < 101 {
        let address = rpc
            .get_new_address(None, None)
            .expect("To be able to get address");

        let remaining_blocks = 101 - i;
        tracing::info!("Not enough blocks mined. Generating {remaining_blocks} blocks");
        rpc.generate_to_address(remaining_blocks, &address)
            .expect("To be able to generate blocks");
    }

    Ok(())
}

async fn fund_everything(faucet: &str, coordinator: &str, maker: &str) -> Result<()> {
    let coordinator = Coordinator::new(init_reqwest(), coordinator);
    let coord_addr = coordinator.get_new_address().await?;
    fund(&coord_addr, Amount::ONE_BTC, faucet).await?;
    let maker = Maker::new(init_reqwest(), maker);
    let maker_addr = maker.get_new_address().await?;
    fund(&maker_addr.to_string(), Amount::ONE_BTC, faucet).await?;
    mine(10, faucet).await?;
    maker
        .sync_on_chain()
        .await
        .expect("to be able to sync on-chain wallet for maker");

    let coordinator_balance = coordinator.get_balance().await?;
    tracing::info!(
        onchain = %coordinator_balance.onchain,
        offchain = %coordinator_balance.offchain,
        "Coordinator balance",
    );

    let coordinator_info = coordinator
        .get_node_info()
        .await
        .expect("To get coordinator's node info");
    maker
        .open_channel(coordinator_info, 10_000_000, None)
        .await
        .expect("To be able to open a channel from maker to coordinator");
    let maker_info = maker.get_node_info().await.expect("To get node info");
    tracing::info!(
        "Opened channel from maker ({}) to coordinator ({})",
        maker_info.pubkey,
        coordinator_info.pubkey
    );

    let node: NodeInfo = coordinator.get_node_info().await?;
    tracing::info!("lightning node: {}", node);

    let lnd_addr: LndAddr = reqwest::get(&format!("{faucet}/lnd/v1/newaddress"))
        .await?
        .json()
        .await?;

    fund(
        &lnd_addr.address,
        Amount::ONE_BTC
            .checked_mul(2)
            .expect("small integers to multiply"),
        faucet,
    )
    .await?;
    mine(10, faucet).await?;

    maker
        .sync_on_chain()
        .await
        .expect("to be able to sync on-chain wallet for maker");
    let maker_balance = maker.get_balance().await?;
    tracing::info!(
        onchain = %maker_balance.onchain,
        offchain = %maker_balance.offchain,
        "Maker balance",
    );

    if let Err(e) = coordinator.sync_wallet().await {
        tracing::warn!("failed to sync coordinator: {}", e);
    }

    let lnd_balance = get_text(&format!("{faucet}/lnd/v1/balance/blockchain")).await?;
    tracing::info!("faucet lightning balance: {}", lnd_balance);

    open_channel(
        &node,
        Amount::ONE_BTC
            .checked_div(10)
            .expect("small integers to divide"),
        faucet,
    )
    .await?;

    // wait until channel has `peer_alias` set correctly
    tracing::info!("Waiting until channel is has correct peer_alias set");
    let mut counter = 0;
    loop {
        if counter == 3 {
            bail!("Could not verify channel is open. Please wipe and try again");
        }
        counter += 1;

        let node_info = get_node_info(faucet).await?;
        if let Some(node_info) = node_info {
            if node_info.num_channels > 0 && node_info.node.alias == "10101.finance" {
                break;
            }
        }

        tracing::info!("Manually broadcasting node announcement and waiting for a few seconds...");
        coordinator.broadcast_node_announcement().await?;
        tokio::time::sleep(Duration::from_secs(5)).await;
    }

    let lnd_channels = get_text(&format!("{faucet}/lnd/v1/channels")).await?;
    tracing::info!("open LND channels: {}", lnd_channels);
    Ok(())
}

#[derive(Deserialize)]
struct LndAddr {
    address: String,
}

async fn get_text(url: &str) -> Result<String> {
    Ok(reqwest::get(url).await?.text().await?)
}

#[derive(Deserialize, Debug)]
struct BitcoindResponse {
    result: String,
}

async fn fund(address: &str, amount: Amount, faucet: &str) -> Result<Response> {
    post_query(
        "bitcoin",
        format!(
            r#"{{"jsonrpc": "1.0", "method": "sendtoaddress", "params": ["{}", "{}"]}}"#,
            address,
            amount.to_btc()
        ),
        faucet,
    )
    .await
}

/// Instructs `bitcoind` to generate to address.
async fn mine(n: u16, faucet: &str) -> Result<()> {
    let response = post_query(
        "bitcoin",
        r#"{"jsonrpc": "1.0", "method": "getnewaddress", "params": []}"#.to_string(),
        faucet,
    )
    .await?;
    let response: BitcoindResponse = response.json().await?;

    post_query(
        "bitcoin",
        format!(
            r#"{{"jsonrpc": "1.0", "method": "generatetoaddress", "params": [{}, "{}"]}}"#,
            n, response.result
        ),
        faucet,
    )
    .await?;
    // For the mined blocks to be picked up by the subsequent wallet syncs
    tokio::time::sleep(Duration::from_secs(5)).await;

    Ok(())
}

async fn post_query(path: &str, body: String, faucet: &str) -> Result<Response> {
    let faucet = faucet.to_string();
    let client = init_reqwest();
    let response = client
        .post(format!("{faucet}/{path}"))
        .body(body)
        .send()
        .await?;

    if !response.status().is_success() {
        bail!(response.text().await?)
    }
    Ok(response)
}

async fn get_query(path: &str, faucet: &str) -> Result<Response> {
    let faucet = faucet.to_string();
    let client = init_reqwest();
    let response = client.get(format!("{faucet}/{path}")).send().await?;

    Ok(response)
}

#[derive(Deserialize, Debug, Clone)]
pub struct LndNodeInfo {
    node: Node,
    num_channels: u32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Node {
    alias: String,
}

async fn get_node_info(faucet: &str) -> Result<Option<LndNodeInfo>> {
    let response = get_query(
        "lnd/v1/graph/node/02dd6abec97f9a748bf76ad502b004ce05d1b2d1f43a9e76bd7d85e767ffb022c9",
        faucet,
    )
    .await?;
    if response.status() == StatusCode::NOT_FOUND {
        tracing::warn!("Node info not yet found.");
        return Ok(None);
    }

    let node_info = response.json().await?;
    Ok(Some(node_info))
}

/// Instructs lnd to open a public channel with the target node.
/// 1. Connect to the target node.
/// 2. Open channel to the target node.
async fn open_channel(node_info: &NodeInfo, amount: Amount, faucet: &str) -> Result<()> {
    // XXX Hacky way of checking whether we need to patch the coordinator
    // address when running locally
    let host = if faucet.to_string().contains("localhost") {
        let port = node_info.address.port();
        let ip_address = local_ip()?;
        let host = format!("{ip_address}:{port}");
        tracing::info!("Running locally, patching host to {host}");
        host
    } else {
        node_info.address.to_string()
    };
    tracing::info!("Connecting lnd to {host}");
    let res = post_query(
        "lnd/v1/peers",
        format!(
            r#"{{"addr": {{ "pubkey": "{}", "host": "{host}" }}, "perm":false }}"]"#,
            node_info.pubkey
        ),
        faucet,
    )
    .await;

    tracing::debug!(?res, "Response after attempting to connect lnd to {host}");

    tokio::time::sleep(Duration::from_secs(5)).await;

    tracing::info!("Opening channel to {} with {amount}", node_info);
    post_query(
        "lnd/v1/channels",
        format!(
            r#"{{"node_pubkey_string":"{}","local_funding_amount":"{}", "min_confs":1 }}"#,
            node_info.pubkey,
            amount.to_sat()
        ),
        faucet,
    )
    .await?;

    mine(10, faucet).await?;

    tracing::info!("connected to channel");

    tracing::info!("You can now use the lightning faucet {faucet}/faucet/");

    // TODO: Inspect the channel manager to wait until channel is usable before returning
    Ok(())
}

// Configure and initialise tracing subsystem
fn init_tracing(level: LevelFilter) -> Result<()> {
    if level == LevelFilter::OFF {
        return Ok(());
    }

    let mut filter = EnvFilter::new("")
        .add_directive(Directive::from(level))
        .add_directive("hyper=warn".parse()?)
        .add_directive("rustls=warn".parse()?)
        .add_directive("reqwest=warn".parse()?);

    // Parse additional log directives from env variable
    let filter = match std::env::var_os(RUST_LOG_ENV).map(|s| s.into_string()) {
        Some(Ok(env)) => {
            for directive in env.split(',') {
                #[allow(clippy::print_stdout)]
                match directive.parse() {
                    Ok(d) => filter = filter.add_directive(d),
                    Err(e) => println!("WARN ignoring log directive: `{directive}`: {e}"),
                };
            }
            filter
        }
        _ => filter,
    };

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_ansi(true);

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .try_init()
        .context("Failed to init tracing")?;

    tracing::info!("Initialized logger");

    Ok(())
}
