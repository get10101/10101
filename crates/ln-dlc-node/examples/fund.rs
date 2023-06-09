use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use bitcoin::Amount;
use clap::Parser;
use ln_dlc_node::node::NodeInfo;
use local_ip_address::local_ip;
use reqwest::Response;
use serde::Deserialize;
use std::time::Duration;
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

    /// Coordinator addres
    #[clap(long, default_value = "http://localhost:8000")]
    pub coordinator: String,
}

#[tokio::main]
async fn main() {
    init_tracing(LevelFilter::DEBUG).expect("tracing to initialise");
    let opts = Opts::parse();
    fund_everything(&opts.faucet, &opts.coordinator)
        .await
        .expect("to be able to fund");
}

async fn fund_everything(faucet: &str, coordinator: &str) -> Result<()> {
    let coord_addr = get_coordinator_address(coordinator).await?;
    fund(&coord_addr, Amount::ONE_BTC, faucet).await?;
    mine(10, faucet).await?;

    let coordinator_balance = get_text(&format!("{coordinator}/api/admin/balance")).await?;
    tracing::info!("coordinator BTC balance: {}", coordinator_balance);

    let node: NodeInfo = reqwest::get(format!("{coordinator}/api/node"))
        .await?
        .json()
        .await?;
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

    let lnd_balance = get_text(&format!("{faucet}/lnd/v1/balance/blockchain")).await?;
    tracing::info!("coordinator lightning balance: {}", lnd_balance);

    open_channel(
        &node,
        Amount::ONE_BTC
            .checked_div(10)
            .expect("small integers to divide"),
        faucet,
    )
    .await?;

    let lnd_channels = get_text(&format!("{faucet}/lnd/v1/channels")).await?;
    tracing::info!("open LND channels: {}", lnd_channels);
    Ok(())
}

#[derive(Deserialize)]
struct LndAddr {
    address: String,
}

// Includes some bespoke text processing that ensures we can deserialise the response properly
async fn get_coordinator_address(coordinator: &str) -> Result<String> {
    Ok(get_text(&format!("{coordinator}/api/newaddress"))
        .await?
        .strip_prefix('"')
        .to_owned()
        .expect("prefix")
        .strip_suffix('"')
        .expect("suffix")
        .to_owned())
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
    let client = reqwest::Client::new();
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
