use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use bitcoin::Address;
use bitcoin::Amount;
use clap::Parser;
use ln_dlc_node::node::NodeInfo;
use local_ip_address::local_ip;
use reqwest::Response;
use reqwest::StatusCode;
use serde::Deserialize;
use std::time::Duration;
use tests_e2e::bitcoind;
use tests_e2e::bitcoind::Bitcoind;
use tests_e2e::coordinator::Coordinator;
use tests_e2e::http::init_reqwest;
use tests_e2e::maker::Maker;
use tests_e2e::wait_until;
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

    /// Coordinator address
    #[clap(long, default_value = "http://localhost:8000")]
    pub coordinator: String,

    /// Maker address
    #[clap(long, default_value = "http://localhost:18000")]
    pub maker: String,

    #[clap(long, default_value = "false")]
    check_peer_alias: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing(LevelFilter::DEBUG).expect("tracing to initialise");
    let opts = Opts::parse();
    let faucet = &opts.faucet;

    let coordinator = Coordinator::new(init_reqwest(), &opts.coordinator);
    fund_everything(faucet, &coordinator, &opts.maker).await?;

    if opts.check_peer_alias {
        check_for_channel_with_peer_alias(faucet, coordinator, "10101.finance").await?;
    }

    let lnd_channels = get_channels(faucet).await?;
    tracing::info!("open LND channels: {:?}", lnd_channels);
    Ok(())
}

async fn fund_everything(faucet: &str, coordinator: &Coordinator, maker: &str) -> Result<()> {
    let client = init_reqwest();
    let coord_addr = coordinator.get_new_address().await?;

    let bitcoind = bitcoind::Bitcoind::new(client, faucet.to_string() + "/bitcoin");

    bitcoind
        .fund(&coord_addr, Amount::ONE_BTC)
        .await
        .context("Could not fund the faucet")?;
    let maker = Maker::new(init_reqwest(), maker);
    let maker_addr = maker.get_new_address().await?;
    bitcoind.fund(&maker_addr, Amount::ONE_BTC).await?;
    bitcoind.mine(10).await?;
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

    bitcoind
        .fund(
            &lnd_addr.address,
            Amount::ONE_BTC
                .checked_mul(10)
                .expect("small integers to multiply"),
        )
        .await?;
    bitcoind.mine(10).await?;

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

    open_channel(&node, Amount::ONE_BTC * 5, faucet, &bitcoind).await?;
    Ok(())
}

/// wait until channel has `peer_alias` set correctly
async fn check_for_channel_with_peer_alias(
    faucet: &str,
    coordinator: Coordinator,
    alias: &str,
) -> Result<()> {
    tracing::info!("Waiting until channel is has correct peer_alias set");

    for _ in 0..2 {
        let node_info = get_node_info(faucet).await?;
        if let Some(node_info) = node_info {
            if node_info.num_channels > 0 && node_info.node.alias == alias {
                tracing::info!("Found channel with peer_alias set to {}", alias);
                return Ok(());
            }
        }
        tracing::info!("Manually broadcasting node announcement and waiting for a few seconds...");
        coordinator.broadcast_node_announcement().await?;
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
    let lnd_channels = get_channels(faucet).await?;
    tracing::info!("open LND channels: {:?}", lnd_channels);
    bail!("Could not verify channel is open. Please wipe and try again");
}

#[derive(Deserialize)]
struct LndAddr {
    address: Address,
}

#[derive(Deserialize, Debug)]
struct LndChannelListResponse {
    channels: Vec<LndChannel>,
}

#[derive(Deserialize, Debug)]
struct LndChannel {
    remote_pubkey: String,
    active: bool,
    peer_alias: String,
    uptime: String,
}

async fn get_text(url: &str) -> Result<String> {
    Ok(reqwest::get(url).await?.text().await?)
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

async fn get_channels(faucet: &str) -> Result<LndChannelListResponse> {
    let response = get_query("lnd/v1/channels", faucet).await?;
    let channels = response.json().await?;
    Ok(channels)
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
async fn open_channel(
    node_info: &NodeInfo,
    amount: Amount,
    faucet: &str,
    bitcoind: &Bitcoind,
) -> Result<()> {
    // Hacky way of checking whether we need to patch the coordinator
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

    tracing::info!("Opening channel to {node_info} with {amount}");
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

    bitcoind.mine(10).await?;

    let lnd_channels = get_channels(faucet).await?;
    tracing::info!("total open LND channels for {faucet}: {:?}", lnd_channels);

    wait_until!(
        lnd_channels
            .channels
            .iter()
            .filter(|c| c.remote_pubkey == node_info.pubkey.to_string())
            .inspect(|c| tracing::debug!(alias = %c.peer_alias, active= %c.active, uptime = %c.uptime, "Found open channel with peer"))
            .any(|c| c.active)
    );

    tracing::info!("You can now use the lightning faucet {faucet}/faucet/");

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
