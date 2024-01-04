use anyhow::Context;
use anyhow::Result;
use bitcoin::Amount;
use clap::Parser;
use fund::bitcoind;
use fund::coordinator::Coordinator;
use fund::http::init_reqwest;
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
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing(LevelFilter::DEBUG).expect("tracing to initialise");
    let opts = Opts::parse();
    fund_everything(&opts.faucet, &opts.coordinator).await
}

async fn fund_everything(faucet: &str, coordinator: &str) -> Result<()> {
    let client = init_reqwest();
    let coordinator = Coordinator::new(client.clone(), coordinator);
    let coord_addr = coordinator.get_new_address().await?;

    let bitcoind = bitcoind::Bitcoind::new(client, faucet.to_string() + "/bitcoin");

    bitcoind
        .fund(&coord_addr, Amount::ONE_BTC)
        .await
        .context("Could not fund the faucet's on-chain wallet")?;
    bitcoind.mine(10).await?;

    coordinator.sync_wallet().await?;

    let coordinator_balance = coordinator.get_balance().await?;
    tracing::info!(
        onchain = %Amount::from_sat(coordinator_balance.onchain),
        offchain = %Amount::from_sat(coordinator_balance.offchain),
        "Coordinator balance",
    );

    let coordinator_node_info = coordinator.get_node_info().await?;
    tracing::info!(?coordinator_node_info);
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
        .add_directive("reqwest=warn".parse()?)
        .add_directive("lightning_transaction_sync=warn".parse()?);

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
