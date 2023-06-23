use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use maker::cli::Opts;
use maker::logger;
use maker::trading;
use std::backtrace::Backtrace;
use std::str::FromStr;
use tracing::level_filters::LevelFilter;

#[tokio::main]
async fn main() -> Result<()> {
    std::panic::set_hook(
        #[allow(clippy::print_stderr)]
        Box::new(|info| {
            let backtrace = Backtrace::force_capture();

            tracing::error!(%info, "Aborting after panic in task");
            eprintln!("{backtrace}");

            std::process::abort()
        }),
    );

    let opts = Opts::read();

    let node_pubkey =
        PublicKey::from_str("03f75f318471d32d39be3c86c622e2c51bd5731bf95f98aaa3ed5d6e1c0025927f")
            .expect("is a valid public key");

    logger::init_tracing(LevelFilter::DEBUG, opts.json)?;

    match trading::run(
        &opts.orderbook,
        node_pubkey,
        opts.network(),
        opts.concurrent_orders,
    )
    .await
    {
        Ok(_) => {
            tracing::error!("Maker stopped trading")
        }
        Err(e) => {
            tracing::error!("Maker stopped trading: {e:#}");
        }
    };

    Ok(())
}
