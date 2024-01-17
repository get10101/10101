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
use rust_embed::RustEmbed;
use std::net::SocketAddr;
use std::time::Duration;
use tower_http::classify::ServerErrorsFailureClass;
use tower_http::trace::TraceLayer;
use tracing::Span;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    // TODO(bonomat): configure the logger properly
    let filter = EnvFilter::new("").add_directive("debug".parse()?);

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .init();

    serve(using_serve_dir(), 3001).await?;

    Ok(())
}

fn using_serve_dir() -> Router {
    Router::new()
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
        )
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
    let mut path = uri.path().trim_start_matches('/').to_string();

    if path.starts_with("assets/") {
        path = path.replace("assets/", "");
    }

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
