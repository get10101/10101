mod api;
mod cli;
mod logger;
mod subscribers;

use crate::api::get_balance;
use crate::api::get_best_quote;
use crate::api::get_node_id;
use crate::api::get_onchain_payment_history;
use crate::api::get_positions;
use crate::api::get_seed_phrase;
use crate::api::get_unused_address;
use crate::api::post_new_order;
use crate::api::send_payment;
use crate::api::version;
use crate::cli::Opts;
use crate::subscribers::AppSubscribers;
use anyhow::Context;
use anyhow::Result;
use axum::http::header;
use axum::http::Request;
use axum::http::StatusCode;
use axum::http::Uri;
use axum::response::Html;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::routing::get;
use axum::routing::post;
use axum::Router;
use bitcoin::Network;
use rust_embed::RustEmbed;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tower_http::classify::ServerErrorsFailureClass;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::level_filters::LevelFilter;
use tracing::Span;

#[tokio::main]
async fn main() -> Result<()> {
    logger::init_tracing(LevelFilter::DEBUG, false, false)?;

    let opts = Opts::read();
    let network = opts.network();

    let data_dir = opts.data_dir()?;
    let data_dir = data_dir.join(network.to_string());
    if !data_dir.exists() {
        std::fs::create_dir_all(&data_dir)
            .context(format!("Could not create data dir for {network}"))?;
    }
    let data_dir = data_dir.clone().to_string_lossy().to_string();
    tracing::info!("Data-dir: {data_dir:?}");

    let coordinator_endpoint = opts.coordinator_endpoint()?;
    let coordinator_p2p_port = opts.coordinator_p2p_port()?;
    let coordinator_pubkey = opts.coordinator_pubkey()?;
    let oracle_endpoint = opts.oracle_endpoint()?;
    let oracle_pubkey = opts.oracle_pubkey()?;
    let coordinator_http_port = opts.coordinator_http_port;
    let esplora_endpoint = opts.esplora;

    let config = native::config::api::Config {
        coordinator_pubkey,
        esplora_endpoint,
        host: coordinator_endpoint,
        p2p_port: coordinator_p2p_port,
        http_port: coordinator_http_port,
        network: network.to_string(),
        oracle_endpoint,
        oracle_pubkey,
        health_check_interval_secs: 60,
        rgs_server_url: None,
    };

    let seed_dir = data_dir.clone();
    native::api::set_config(config, data_dir.clone(), seed_dir.clone()).expect("to set config");

    let _handle = tokio::task::spawn_blocking({
        let seed_dir = seed_dir.clone();
        move || native::api::run_in_test(seed_dir).expect("to start backend")
    })
    .await;

    let (rx, tx) = AppSubscribers::new().await;
    native::event::subscribe(tx);

    serve(using_serve_dir(Arc::new(rx), network), 3001).await?;

    Ok(())
}

fn using_serve_dir(subscribers: Arc<AppSubscribers>, network: Network) -> Router {
    let router = Router::new()
        .route("/", get(index_handler))
        .route("/api/version", get(version))
        .route("/api/balance", get(get_balance))
        .route("/api/newaddress", get(get_unused_address))
        .route("/api/sendpayment", post(send_payment))
        .route("/api/history", get(get_onchain_payment_history))
        .route("/api/orders", post(post_new_order))
        .route("/api/positions", get(get_positions))
        .route("/api/quotes/:contract_symbol", get(get_best_quote))
        .route("/api/node", get(get_node_id))
        .route("/api/seed", get(get_seed_phrase))
        .route("/main.dart.js", get(main_dart_handler))
        .route("/flutter.js", get(flutter_js))
        .route("/index.html", get(index_handler))
        .route("/assets/*file", get(static_handler))
        .fallback_service(get(not_found))
        .layer(
            TraceLayer::new_for_http()
                .on_request(|request: &Request<axum::body::Body>, _span: &Span| {
                    tracing::debug!(
                        method = request.method().to_string(),
                        uri = request.uri().path(),
                        "request"
                    )
                })
                .on_response(())
                .on_body_chunk(())
                .on_eos(())
                .on_failure(
                    |error: ServerErrorsFailureClass, _latency: Duration, _span: &Span| {
                        tracing::error!("something went wrong : {error:#}")
                    },
                ),
        )
        .with_state(subscribers);

    if matches!(
        network,
        Network::Regtest | Network::Signet | Network::Testnet
    ) {
        router.layer(CorsLayer::permissive())
    } else {
        router
    }
}

// We use static route matchers ("/" and "/index.html") to serve our home
// page.
async fn index_handler() -> impl IntoResponse {
    let result = "/index.html".parse::<Uri>().expect("to be a valid uri");
    static_handler(result).await
}

// We use static route matchers ("/main_dart.js") to serve our js library
async fn main_dart_handler() -> impl IntoResponse {
    static_handler("/main.dart.js".parse::<Uri>().expect("to be a valid uri")).await
}

// We use static route matchers ("/flutter.js") to serve our js library
async fn flutter_js() -> impl IntoResponse {
    static_handler("/flutter.js".parse::<Uri>().expect("to be a valid uri")).await
}

// We use a wildcard matcher ("/dist/*file") to match against everything
// within our defined assets directory. This is the directory on our Asset
// struct below, where folder = "examples/public/".
async fn static_handler(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/').to_string();
    StaticFile(path)
}

// Finally, we use a fallback route for anything that didn't match.
async fn not_found() -> Html<&'static str> {
    Html("<h1>404</h1><p>Not Found</p>")
}

#[derive(RustEmbed)]
#[folder = "frontend/build/web"]
struct Asset;

pub struct StaticFile<T>(pub T);

impl<T> IntoResponse for StaticFile<T>
where
    T: Into<String>,
{
    fn into_response(self) -> Response {
        let path = self.0.into();

        match Asset::get(path.as_str()) {
            Some(content) => {
                let mime = mime_guess::from_path(path).first_or_octet_stream();
                ([(header::CONTENT_TYPE, mime.as_ref())], content.data).into_response()
            }
            None => (StatusCode::NOT_FOUND, "404 Not Found").into_response(),
        }
    }
}

async fn serve(app: Router, port: u16) -> Result<()> {
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::debug!("listening on {}", listener.local_addr()?);
    axum::serve(listener, app).await?;
    Ok(())
}
