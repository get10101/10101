mod api;
mod auth;
mod cli;
mod logger;
mod session;
mod subscribers;

use crate::auth::Backend;
use crate::cli::Opts;
use crate::session::InMemorySessionStore;
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
use axum::Router;
use axum_login::login_required;
use axum_login::tower_sessions::Expiry;
use axum_login::tower_sessions::SessionManagerLayer;
use axum_login::AuthManagerLayerBuilder;
use axum_server::tls_rustls::RustlsConfig;
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

    let cert_dir = opts.cert_dir()?;
    tracing::info!("Cert-dir: {cert_dir:?}");

    let coordinator_endpoint = opts.coordinator_endpoint()?;
    let coordinator_p2p_port = opts.coordinator_p2p_port()?;
    let coordinator_pubkey = opts.coordinator_pubkey()?;
    let oracle_endpoint = opts.oracle_endpoint()?;
    let oracle_pubkey = opts.oracle_pubkey()?;
    let password = opts.password();
    let coordinator_http_port = opts.coordinator_http_port;
    let esplora_endpoint = opts.esplora;
    let secure = opts.secure;

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

    // configure certificate and private key used by https
    let config =
        RustlsConfig::from_pem_file(cert_dir.join("cert.pem"), cert_dir.join("key.pem")).await?;

    let session_store = InMemorySessionStore::new();
    let deletion_task = tokio::task::spawn(
        session_store
            .clone()
            .continuously_delete_expired(Duration::from_secs(60)),
    );

    let session_layer = SessionManagerLayer::new(session_store.clone())
        .with_secure(matches!(network, Network::Bitcoin))
        .with_expiry(Expiry::OnInactivity(time::Duration::hours(1)));

    let auth_layer = AuthManagerLayerBuilder::new(
        Backend {
            hashed_password: password,
        },
        session_layer,
    )
    .build();

    let app = api::router(Arc::new(rx))
        .route_layer(login_required!(Backend))
        .merge(auth::router())
        .merge(router(network))
        .layer(auth_layer);

    // run https server
    let addr = SocketAddr::from(([0, 0, 0, 0], 3001));
    tracing::debug!("listening on {}", addr);
    match secure {
        false => {
            let listener = tokio::net::TcpListener::bind(addr).await?;
            axum::serve(listener, app.into_make_service()).await
        }
        true => {
            axum_server::bind_rustls(addr, config)
                .serve(app.into_make_service())
                .await
        }
    }?;

    deletion_task.await??;

    Ok(())
}

fn router(network: Network) -> Router {
    let router = Router::new()
        .route("/", get(index_handler))
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
        );

    if matches!(network, Network::Bitcoin) {
        router
    } else {
        router.layer(CorsLayer::very_permissive())
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
