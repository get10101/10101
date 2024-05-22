use anyhow::Context;
use anyhow::Result;
use axum::extract::ws::Message;
use axum::extract::ws::WebSocketUpgrade;
use axum::extract::Extension;
use axum::extract::Json;
use axum::extract::Query;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::routing::get;
use axum::routing::post;
use axum::Router;
use lnd_bridge::CancelInvoice;
use lnd_bridge::Invoice;
use lnd_bridge::InvoiceParams;
use lnd_bridge::InvoiceResponse;
use lnd_bridge::InvoiceResult;
use lnd_bridge::InvoiceState;
use lnd_bridge::SettleInvoice;
use serde::Deserialize;
use std::net::SocketAddr;
use time::macros::format_description;
use tokio::sync::broadcast;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::filter::Directive;
use tracing_subscriber::fmt::time::UtcTime;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::Layer;

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing(LevelFilter::DEBUG)?;
    let (tx, _rx) = broadcast::channel::<String>(100);

    // Build the Axum router
    let app = Router::new()
        .route("/v2/invoices/subscribe/:r_hash", get(subscribe_invoices))
        .route("/v2/invoices/hodl", post(create_invoice))
        .route("/v2/invoices/settle", post(settle_invoice))
        .route("/v2/invoices/cancel", post(cancel_invoice))
        .route("/pay_invoice", post(pay_invoice))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(Extension(tx.clone())),
        );

    let addr = SocketAddr::from(([0, 0, 0, 0], 18080));
    tracing::info!("Listening on http://{}", addr);

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

async fn create_invoice(
    Extension(tx): Extension<broadcast::Sender<String>>,
    headers: HeaderMap,
    Json(params): Json<InvoiceParams>,
) -> impl IntoResponse {
    match headers.get("Grpc-Metadata-macaroon") {
        Some(_) => {
            let payment_request = "lntbs101010n1pnyw6wppp59sgej6qrv25s6k7y4eg2e43kqjywhlfd9knk0kvuuluv0jzlx4jqdp8ge6kuepq09hh2u3qxycrzvp3ypcx7umfw35k7mscqzzsxqzfvsp5zuekkq7kfall8gkfu4a9f8d90nma7z2hhe026kka4k7tfnpekamq9qxpqysgq265wu2x0hrujk2lyuhftqa9drpte8tp69gd5jehjxqyq526c9ayzy2zyx9eeacj0zvmnz874e59th37un8w280q8dyc5y2pjyy6c6ngqgp78j3".to_string();
            let payment_addr = "LBGZaANiqQ1bxK5QrNY2BIjr/S0tp2fZnOf4x8hfNWQ=".to_string();
            let response = InvoiceResponse {
                add_index: 1,
                payment_addr: payment_addr.clone(),
                payment_request: payment_request.clone(),
            };

            let result = InvoiceResult {
                result: Invoice {
                    memo: params.memo,
                    expiry: params.expiry,
                    amt_paid_sat: 0,
                    state: InvoiceState::Open,
                    payment_request,
                    r_hash: payment_addr,
                    add_index: 1,
                    settle_index: 2,
                },
            };

            let message = serde_json::to_string(&result).expect("to serialize");

            let _ = tx.send(message);

            (StatusCode::OK, Json(response)).into_response()
        }
        None => Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .body::<String>("Missing macaroon".into())
            .expect("body")
            .into_response(),
    }
}

async fn settle_invoice(
    Extension(tx): Extension<broadcast::Sender<String>>,
    headers: HeaderMap,
    Json(_): Json<SettleInvoice>,
) -> impl IntoResponse {
    match headers.get("Grpc-Metadata-macaroon") {
        Some(_) => {
            let message = serde_json::to_string(&InvoiceResult {
                result: Invoice {
                    memo: "".to_string(),
                    expiry: 0,
                    amt_paid_sat: 0,
                    state: InvoiceState::Settled,
                    payment_request: "".to_string(),
                    r_hash: "".to_string(),
                    add_index: 0,
                    settle_index: 0,
                },
            })
            .expect("to serialize");

            let _ = tx.send(message);

            (StatusCode::OK, Json(())).into_response()
        }
        None => Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .body::<String>("Missing macaroon".into())
            .expect("body")
            .into_response(),
    }
}

async fn cancel_invoice(
    Extension(tx): Extension<broadcast::Sender<String>>,
    headers: HeaderMap,
    Json(_): Json<CancelInvoice>,
) -> impl IntoResponse {
    match headers.get("Grpc-Metadata-macaroon") {
        Some(_) => {
            let message = serde_json::to_string(&InvoiceResult {
                result: Invoice {
                    memo: "".to_string(),
                    expiry: 0,
                    amt_paid_sat: 0,
                    state: InvoiceState::Canceled,
                    payment_request: "".to_string(),
                    r_hash: "".to_string(),
                    add_index: 0,
                    settle_index: 0,
                },
            })
            .expect("to serialize");

            let _ = tx.send(message);

            (StatusCode::OK, Json(())).into_response()
        }
        None => Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .body::<String>("Missing macaroon".into())
            .expect("body")
            .into_response(),
    }
}

async fn pay_invoice(Extension(tx): Extension<broadcast::Sender<String>>) -> impl IntoResponse {
    let message = serde_json::to_string(&InvoiceResult {
        result: Invoice {
            memo: "".to_string(),
            expiry: 0,
            amt_paid_sat: 0,
            state: InvoiceState::Accepted,
            payment_request: "".to_string(),
            r_hash: "".to_string(),
            add_index: 0,
            settle_index: 0,
        },
    })
    .expect("to serialize");
    let _ = tx.send(message);

    StatusCode::OK
}

#[derive(Deserialize)]
struct SubscribeQuery {
    settle_index: Option<u64>,
}

async fn subscribe_invoices(
    ws: WebSocketUpgrade,
    Query(params): Query<SubscribeQuery>,
    Extension(tx): Extension<broadcast::Sender<String>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    match headers.get("Grpc-Metadata-macaroon") {
        Some(_) => ws.on_upgrade(move |socket| handle_socket(socket, tx, params.settle_index)),
        None => Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .body::<String>("Missing macaroon".into())
            .expect("body")
            .into_response(),
    }
}

async fn handle_socket(
    mut socket: axum::extract::ws::WebSocket,
    tx: broadcast::Sender<String>,
    _settle_index: Option<u64>,
) {
    let mut rx = tx.subscribe();

    tokio::spawn({
        async move {
            while let Ok(msg) = rx.recv().await {
                match serde_json::from_str::<InvoiceResult>(&msg) {
                    Ok(invoice) => {
                        if let Err(e) = socket.send(Message::Text(msg)).await {
                            tracing::error!("Failed to send msg on socket. Error: {e:#}");
                        }

                        if matches!(
                            invoice.result.state,
                            InvoiceState::Canceled | InvoiceState::Settled
                        ) {
                            return;
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to parse msg. Error: {e:#}");
                    }
                }
            }
        }
    });
}

const RUST_LOG_ENV: &str = "RUST_LOG";

pub fn init_tracing(level: LevelFilter) -> Result<()> {
    if level == LevelFilter::OFF {
        return Ok(());
    }

    let mut filter = EnvFilter::new("")
        .add_directive(Directive::from(level))
        .add_directive("hyper=warn".parse()?);

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

    let fmt_layer = fmt_layer
        .with_timer(UtcTime::new(format_description!(
            "[year]-[month]-[day] [hour]:[minute]:[second]"
        )))
        .boxed();

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .try_init()
        .context("Failed to init tracing")?;

    tracing::info!("Initialized logger");

    Ok(())
}
