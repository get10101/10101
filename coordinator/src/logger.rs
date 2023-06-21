use anyhow::Context;
use anyhow::Result;
use time::macros::format_description;
use tracing::metadata::LevelFilter;
use tracing_subscriber::filter::Directive;
use tracing_subscriber::fmt::time::UtcTime;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::Layer;

const RUST_LOG_ENV: &str = "RUST_LOG";

// Configure and initialise tracing subsystem
pub fn init_tracing(level: LevelFilter, json_format: bool, tokio_console: bool) -> Result<()> {
    if level == LevelFilter::OFF {
        return Ok(());
    }

    let is_terminal = atty::is(atty::Stream::Stderr);

    let filter = EnvFilter::new("")
        .add_directive(Directive::from(level))
        .add_directive("hyper=warn".parse()?)
        .add_directive("rustls=warn".parse()?)
        .add_directive("sled=warn".parse()?)
        .add_directive("bdk=warn".parse()?) // bdk is quite spamy on debug
        .add_directive("lightning=trace".parse()?)
        .add_directive("ureq=info".parse()?);

    let mut filter = if tokio_console {
        filter
            .add_directive("tokio=trace".parse()?)
            .add_directive("runtime=trace".parse()?)
    } else {
        filter
    };

    let console_layer = if tokio_console {
        Some(
            console_subscriber::ConsoleLayer::builder()
                .server_addr(([0, 0, 0, 0], 6669))
                .spawn(),
        )
    } else {
        None
    };

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
        .with_ansi(is_terminal);

    let fmt_layer = if json_format {
        fmt_layer.json().with_timer(UtcTime::rfc_3339()).boxed()
    } else {
        fmt_layer
            .with_timer(UtcTime::new(format_description!(
                "[year]-[month]-[day] [hour]:[minute]:[second]"
            )))
            .boxed()
    };

    tracing_subscriber::registry()
        .with(filter)
        .with(console_layer)
        .with(fmt_layer)
        .try_init()
        .context("Failed to init tracing")?;

    tracing::info!("Initialized logger");

    Ok(())
}
