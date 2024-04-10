mod api;
mod auth;
mod cli;
mod logger;
mod session;
mod subscribers;

use crate::api::version;
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
use bitcoin::Network;
use hyper::body::Incoming;
use hyper_util::rt::TokioExecutor;
use hyper_util::rt::TokioIo;
use rust_embed::RustEmbed;
use rustls_pemfile::certs;
use rustls_pemfile::pkcs8_private_keys;
use std::fs::File;
use std::io::BufReader;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio_rustls::rustls::Certificate;
use tokio_rustls::rustls::PrivateKey;
use tokio_rustls::rustls::ServerConfig;
use tokio_rustls::TlsAcceptor;
use tower::Service;
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
    let electrs_endpoint = opts.electrs;
    let secure = opts.secure;

    let config = native::config::api::Config {
        coordinator_pubkey,
        electrs_endpoint,
        host: coordinator_endpoint,
        p2p_port: coordinator_p2p_port,
        http_port: coordinator_http_port,
        network: network.to_string(),
        oracle_endpoint,
        oracle_pubkey,
        health_check_interval_secs: 60,
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

    let app_state = AppState {
        whitelist_withdrawal_addresses: opts.whitelist_withdrawal_addresses,
        withdrawal_addresses: opts.withdrawal_address,
        subscribers: Arc::new(rx),
    };

    let app = api::router(app_state)
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
            // configure certificate and private key used by https
            let rustls_config =
                rustls_server_config(cert_dir.join("key.pem"), cert_dir.join("cert.pem"))?;

            let tls_acceptor = TlsAcceptor::from(rustls_config);

            let listener = tokio::net::TcpListener::bind(addr).await?;

            loop {
                let tower_service = app.clone();
                let tls_acceptor = tls_acceptor.clone();

                // Wait for new tcp connection
                let (cnx, addr) = listener.accept().await?;

                tokio::spawn(async move {
                    // Wait for tls handshake to happen
                    let Ok(stream) = tls_acceptor.accept(cnx).await else {
                        tracing::error!("error during tls handshake connection from {}", addr);
                        return;
                    };

                    // Hyper has its own `AsyncRead` and `AsyncWrite` traits and doesn't use tokio.
                    // `TokioIo` converts between them.
                    let stream = TokioIo::new(stream);

                    // Hyper also has its own `Service` trait and doesn't use tower. We can use
                    // `hyper::service::service_fn` to create a hyper `Service` that calls our app
                    // through `tower::Service::call`.
                    let hyper_service =
                        hyper::service::service_fn(move |request: Request<Incoming>| {
                            // We have to clone `tower_service` because hyper's `Service` uses
                            // `&self` whereas tower's `Service`
                            // requires `&mut self`.
                            //
                            // We don't need to call `poll_ready` since `Router` is always ready.
                            tower_service.clone().call(request)
                        });

                    let ret = hyper_util::server::conn::auto::Builder::new(TokioExecutor::new())
                        .serve_connection_with_upgrades(stream, hyper_service)
                        .await;

                    if let Err(err) = ret {
                        tracing::warn!("error serving connection from {}: {}", addr, err);
                    }
                });
            }
        }
    }?;

    deletion_task.await??;

    Ok(())
}

pub struct AppState {
    pub whitelist_withdrawal_addresses: bool,
    pub withdrawal_addresses: Vec<String>,
    pub subscribers: Arc<AppSubscribers>,
}

fn router(network: Network) -> Router {
    let router = Router::new()
        .route("/", get(index_handler))
        .route("/main.dart.js", get(main_dart_handler))
        .route("/flutter.js", get(flutter_js))
        .route("/index.html", get(index_handler))
        .route("/assets/*file", get(static_handler))
        .route("/api/version", get(version))
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

fn rustls_server_config(
    key: impl AsRef<Path>,
    cert: impl AsRef<Path>,
) -> Result<Arc<ServerConfig>> {
    let mut key_reader = BufReader::new(File::open(key)?);
    let mut cert_reader = BufReader::new(File::open(cert)?);

    let key = PrivateKey(pkcs8_private_keys(&mut key_reader)?.remove(0));
    let certs = certs(&mut cert_reader)?
        .into_iter()
        .map(Certificate)
        .collect();

    let mut config = ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .expect("bad certificate/key");

    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    Ok(Arc::new(config))
}
