use crate::backup::SledBackup;
use crate::campaign::post_push_campaign;
use crate::collaborative_revert::confirm_collaborative_revert;
use crate::db;
use crate::db::user;
use crate::db::user::User;
use crate::leaderboard::generate_leader_board;
use crate::leaderboard::LeaderBoard;
use crate::leaderboard::LeaderBoardCategory;
use crate::leaderboard::LeaderBoardQueryParams;
use crate::message::NewUserMessage;
use crate::message::OrderbookMessage;
use crate::node::invoice;
use crate::node::Node;
use crate::notifications::Notification;
use crate::orderbook::trading::NewOrderMessage;
use crate::parse_dlc_channel_id;
use crate::settings::Settings;
use crate::trade::websocket::InternalPositionUpdateMessage;
use crate::AppError;
use admin::close_channel;
use admin::collaborative_revert;
use admin::delete_dlc_channel;
use admin::get_balance;
use admin::get_fee_rate_estimation;
use admin::get_settings;
use admin::get_user_referral_status;
use admin::get_utxos;
use admin::is_connected;
use admin::list_dlc_channels;
use admin::list_on_chain_transactions;
use admin::list_peers;
use admin::migrate_dlc_channels;
use admin::post_sync;
use admin::resend_renew_revoke_message;
use admin::roll_back_dlc_channel;
use admin::rollover;
use admin::update_settings;
use anyhow::Result;
use axum::extract::ConnectInfo;
use axum::extract::DefaultBodyLimit;
use axum::extract::Path;
use axum::extract::Query;
use axum::extract::State;
use axum::extract::WebSocketUpgrade;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::delete;
use axum::routing::get;
use axum::routing::post;
use axum::routing::put;
use axum::Json;
use axum::Router;
use bitcoin::consensus::encode::serialize_hex;
use bitcoin::secp256k1::ecdsa::Signature;
use bitcoin::secp256k1::PublicKey;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::secp256k1::VerifyOnly;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::PgConnection;
use lnd_bridge::InvoiceParams;
use lnd_bridge::LndBridge;
use opentelemetry_prometheus::PrometheusExporter;
use orderbook::delete_order;
use orderbook::get_order;
use orderbook::get_orders;
use orderbook::post_order;
use orderbook::websocket_handler;
use prometheus::Encoder;
use prometheus::TextEncoder;
use serde::Serialize;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use time::macros::format_description;
use time::Date;
use time::OffsetDateTime;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::sync::RwLock;
use tracing::instrument;
use xxi_node::commons;
use xxi_node::commons::Backup;
use xxi_node::commons::CollaborativeRevertTraderResponse;
use xxi_node::commons::DeleteBackup;
use xxi_node::commons::Message;
use xxi_node::commons::Poll;
use xxi_node::commons::PollAnswers;
use xxi_node::commons::RegisterParams;
use xxi_node::commons::ReportedError;
use xxi_node::commons::Restore;
use xxi_node::commons::SignedValue;
use xxi_node::commons::UpdateUsernameParams;
use xxi_node::node::NodeInfo;

mod admin;
mod orderbook;

pub struct AppState {
    pub node: Node,
    // Channel used to send messages to all connected clients.
    pub tx_orderbook_feed: broadcast::Sender<Message>,
    /// A channel used to send messages about position updates
    pub tx_position_feed: broadcast::Sender<InternalPositionUpdateMessage>,
    pub tx_user_feed: broadcast::Sender<NewUserMessage>,
    pub trading_sender: mpsc::Sender<NewOrderMessage>,
    pub pool: Pool<ConnectionManager<PgConnection>>,
    pub settings: RwLock<Settings>,
    pub exporter: PrometheusExporter,
    pub node_alias: String,
    pub auth_users_notifier: mpsc::Sender<OrderbookMessage>,
    pub notification_sender: mpsc::Sender<Notification>,
    pub user_backup: SledBackup,
    pub secp: Secp256k1<VerifyOnly>,
    pub lnd_bridge: LndBridge,
}

#[allow(clippy::too_many_arguments)]
pub fn router(
    node: Node,
    pool: Pool<ConnectionManager<PgConnection>>,
    settings: Settings,
    exporter: PrometheusExporter,
    node_alias: &str,
    trading_sender: mpsc::Sender<NewOrderMessage>,
    tx_orderbook_feed: broadcast::Sender<Message>,
    tx_position_feed: broadcast::Sender<InternalPositionUpdateMessage>,
    tx_user_feed: broadcast::Sender<NewUserMessage>,
    auth_users_notifier: mpsc::Sender<OrderbookMessage>,
    notification_sender: mpsc::Sender<Notification>,
    user_backup: SledBackup,
    lnd_bridge: LndBridge,
) -> Router {
    let secp = Secp256k1::verification_only();

    let app_state = Arc::new(AppState {
        node,
        pool,
        settings: RwLock::new(settings),
        tx_orderbook_feed,
        tx_position_feed,
        tx_user_feed,
        trading_sender,
        exporter,
        node_alias: node_alias.to_string(),
        auth_users_notifier,
        notification_sender,
        user_backup,
        secp,
        lnd_bridge,
    });

    Router::new()
        .route("/", get(lightning_peer_ws_handler))
        .route("/api/version", get(version))
        .route("/api/polls", post(post_poll_answer))
        .route("/api/polls/:node_id", get(get_polls))
        .route(
            "/api/fee_rate_estimate/:target",
            get(get_fee_rate_estimation),
        )
        .route("/api/backup/:node_id", post(back_up).delete(delete_backup))
        .route("/api/restore/:node_id", get(restore))
        .route("/api/newaddress", get(get_unused_address))
        .route("/api/node", get(get_node_info))
        .route("/api/orderbook/orders", get(get_orders).post(post_order))
        .route("/api/orderbook/orders/:order_id", get(get_order))
        .route("/api/orderbook/websocket", get(websocket_handler))
        .route("/api/invoice", post(create_invoice))
        .route("/api/users", post(post_register))
        .route("/api/users/:trader_pubkey", get(get_user))
        .route("/api/users/nickname", put(update_nickname))
        .route("/api/report-error", post(post_error))
        // TODO: we should move this back into public once we add signing to this function
        .route(
            "/api/admin/orderbook/orders/:order_id",
            delete(delete_order),
        )
        .route("/api/admin/rollover/:dlc_channel_id", post(rollover))
        .route("/api/admin/wallet/balance", get(get_balance))
        .route("/api/admin/wallet/utxos", get(get_utxos))
        .route("/api/admin/channels/:channel_id", delete(close_channel))
        .route("/api/admin/peers", get(list_peers))
        .route("/api/admin/dlc_channels", get(list_dlc_channels))
        .route(
            "/api/admin/dlc_channels/:channel_id",
            delete(delete_dlc_channel),
        )
        .route(
            "/api/admin/dlc_channels/rollback/:channel_id",
            post(roll_back_dlc_channel),
        )
        .route("/api/admin/transactions", get(list_on_chain_transactions))
        .route("/api/admin/channels/revert", post(collaborative_revert))
        .route(
            "/api/channels/confirm-collab-revert",
            post(collaborative_revert_confirm),
        )
        .route("/api/admin/is_connected/:target_pubkey", get(is_connected))
        .route(
            "/api/admin/settings",
            get(get_settings).put(update_settings),
        )
        .route("/api/admin/sync", post(post_sync))
        .route("/api/admin/campaign/push", post(post_push_campaign))
        .route(
            "/api/admin/resend_renew_revoke_message/:trader_pubkey",
            post(resend_renew_revoke_message),
        )
        .route(
            "/api/admin/migrate_dlc_channels",
            post(migrate_dlc_channels),
        )
        .route(
            "/api/admin/users/:trader_pubkey/referrals",
            get(get_user_referral_status),
        )
        .route("/metrics", get(get_metrics))
        .route("/health", get(get_health))
        .route("/api/leaderboard", get(get_leaderboard))
        .route(
            "/api/admin/trade/websocket",
            get(crate::trade::websocket::websocket_handler),
        )
        .layer(DefaultBodyLimit::disable())
        .layer(DefaultBodyLimit::max(50 * 1024))
        .with_state(app_state)
}

#[derive(serde::Serialize)]
struct HelloWorld {
    hello: String,
}

pub async fn lightning_peer_ws_handler(
    ws: Option<WebSocketUpgrade>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match ws {
        Some(ws) => {
            let peer_manager = state.node.inner.peer_manager.clone();
            ws.on_upgrade(move |socket| {
                xxi_node::networking::axum::setup_inbound(peer_manager, socket, addr)
            })
            .into_response()
        }
        None => Json(HelloWorld {
            hello: "world".to_string(),
        })
        .into_response(),
    }
}

pub async fn get_unused_address(
    State(app_state): State<Arc<AppState>>,
) -> Result<String, AppError> {
    let address = app_state.node.inner.get_unused_address().map_err(|e| {
        AppError::InternalServerError(format!("Could not get unused address: {e:#}"))
    })?;

    Ok(address.to_string())
}

#[instrument(skip_all, err(Debug))]
pub async fn get_node_info(
    State(app_state): State<Arc<AppState>>,
) -> Result<Json<NodeInfo>, AppError> {
    let node_info = app_state.node.inner.info;
    Ok(Json(node_info))
}

#[instrument(skip_all, err(Debug))]
pub async fn post_register(
    State(state): State<Arc<AppState>>,
    params: Json<RegisterParams>,
) -> Result<(), AppError> {
    let register_params = params.0;
    tracing::info!(?register_params, "Registered new user");

    let mut conn = state
        .pool
        .get()
        .map_err(|e| AppError::InternalServerError(format!("Could not get connection: {e:#}")))?;

    user::upsert_user(
        &mut conn,
        register_params.pubkey,
        register_params.contact.clone(),
        register_params.nickname.clone(),
        register_params.version.clone(),
        register_params.os,
        register_params.referral_code,
    )
    .map_err(|e| AppError::InternalServerError(format!("Could not upsert user: {e:#}")))?;

    Ok(())
}

#[instrument(skip_all, err(Debug))]
pub async fn update_nickname(
    State(state): State<Arc<AppState>>,
    params: Json<UpdateUsernameParams>,
) -> Result<(), AppError> {
    let register_params = params.0;
    tracing::info!(?register_params, "Updating user's nickname");

    let mut conn = state
        .pool
        .get()
        .map_err(|e| AppError::InternalServerError(format!("Could not get connection: {e:#}")))?;

    user::update_nickname(&mut conn, register_params.pubkey, register_params.nickname)
        .map_err(|e| AppError::InternalServerError(format!("Could not update nickname: {e:#}")))?;

    Ok(())
}

impl TryFrom<User> for commons::User {
    type Error = AppError;
    fn try_from(value: User) -> Result<Self, Self::Error> {
        Ok(commons::User {
            pubkey: PublicKey::from_str(&value.pubkey).map_err(|_| {
                AppError::InternalServerError("Could not parse user pubkey".to_string())
            })?,
            contact: Some(value.contact).filter(|s| !s.is_empty()),
            nickname: value.nickname,
            referral_code: value.referral_code,
        })
    }
}

#[instrument(skip_all, err(Debug))]
pub async fn get_user(
    State(state): State<Arc<AppState>>,
    Path(trader_pubkey): Path<String>,
) -> Result<Json<commons::User>, AppError> {
    let mut conn = state
        .pool
        .get()
        .map_err(|e| AppError::InternalServerError(format!("Could not get connection: {e:#}")))?;

    let trader_pubkey = PublicKey::from_str(trader_pubkey.as_str())
        .map_err(|_| AppError::BadRequest("Invalid trader id provided".to_string()))?;

    let option = user::get_user(&mut conn, &trader_pubkey)
        .map_err(|e| AppError::InternalServerError(format!("Could not load users: {e:#}")))?;

    match option {
        None => Err(AppError::BadRequest("No user found".to_string())),
        Some(user) => Ok(Json(user.try_into()?)),
    }
}

pub async fn get_metrics(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let exporter = state.exporter.clone();
    let encoder = TextEncoder::new();
    let metric_families = exporter.registry().gather();
    let mut result = vec![];
    match encoder.encode(&metric_families, &mut result) {
        Ok(()) => (),
        Err(err) => {
            tracing::error!("Could not collect opentelemetry metrics {err:#}");
            return (StatusCode::INTERNAL_SERVER_ERROR, format!("{:?}", err));
        }
    };

    let open_telemetry_metrics = match String::from_utf8(result) {
        Ok(s) => s,
        Err(err) => {
            tracing::error!("Could not format metrics as string {err:#}");
            return (StatusCode::INTERNAL_SERVER_ERROR, format!("{:?}", err));
        }
    };

    (StatusCode::OK, open_telemetry_metrics)
}

pub async fn get_health() -> Result<Json<String>, AppError> {
    // TODO: Implement any health check logic we'd need
    // So far this just returns if the server is running
    Ok(Json("Server is healthy".to_string()))
}

#[derive(Serialize)]
pub struct Version {
    version: String,
    commit_hash: String,
    branch: String,
}

pub async fn version() -> Result<Json<Version>, AppError> {
    Ok(Json(Version {
        version: env!("CARGO_PKG_VERSION").to_string(),
        commit_hash: env!("COMMIT_HASH").to_string(),
        branch: env!("BRANCH_NAME").to_string(),
    }))
}

pub async fn get_polls(
    Path(node_id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<Poll>>, AppError> {
    let node_id = PublicKey::from_str(&node_id)
        .map_err(|e| AppError::BadRequest(format!("Invalid node id provided. {e:#}")))?;

    let mut connection = state
        .pool
        .get()
        .map_err(|_| AppError::InternalServerError("Could not get db connection".to_string()))?;
    let polls = db::polls::active(&mut connection, &node_id).map_err(|error| {
        AppError::InternalServerError(format!("Could not fetch new polls {error}"))
    })?;
    Ok(Json(polls))
}

pub async fn post_poll_answer(
    State(state): State<Arc<AppState>>,
    poll_answer: Json<PollAnswers>,
) -> Result<(), AppError> {
    tracing::trace!(
            poll_id = poll_answer.poll_id,
            trader_pk = poll_answer.trader_pk.to_string(),
            answers = ?poll_answer.answers,
        "Received new answer");
    let mut connection = state
        .pool
        .get()
        .map_err(|_| AppError::InternalServerError("Could not get db connection".to_string()))?;

    db::polls::add_answer(&mut connection, poll_answer.0).map_err(|error| {
        AppError::InternalServerError(format!("Could not save answer in db: {error:?}"))
    })?;

    Ok(())
}

#[instrument(skip_all, err(Debug))]
pub async fn collaborative_revert_confirm(
    State(state): State<Arc<AppState>>,
    revert_params: Json<CollaborativeRevertTraderResponse>,
) -> Result<Json<String>, AppError> {
    let mut conn = state.pool.clone().get().map_err(|error| {
        AppError::InternalServerError(format!("Could not acquire db lock {error:#}"))
    })?;

    let channel_id_string = revert_params.channel_id.clone();
    let channel_id = parse_dlc_channel_id(channel_id_string.as_str()).map_err(|error| {
        tracing::error!(
            channel_id = channel_id_string,
            "Invalid channel id provided. {error:#}"
        );
        AppError::BadRequest("Invalid channel id provided".to_string())
    })?;

    tracing::info!(
        channel_id = channel_id_string,
        "Confirming collaborative channel revert"
    );
    let inner_node = state.node.inner.clone();

    let raw_tx = confirm_collaborative_revert(
        inner_node,
        &mut conn,
        channel_id,
        revert_params.transaction.clone(),
        revert_params.signature,
    )
    .map_err(|error| {
        tracing::error!(
            channel_id = channel_id_string,
            "Could not confirm collaborative revert: {error:#}"
        );
        AppError::InternalServerError("Could not confirm collaborative revert".to_string())
    })?;
    Ok(Json(serialize_hex(&raw_tx)))
}

// TODO(holzeis): There is no reason the backup and restore api has to run on the coordinator. On
// the contrary it would be much more reasonable to have the backup and restore api run separately.
#[instrument(skip_all, err(Debug))]
pub async fn back_up(
    Path(node_id): Path<String>,
    State(state): State<Arc<AppState>>,
    backup: Json<Backup>,
) -> Result<(), AppError> {
    let node_id = PublicKey::from_str(&node_id)
        .map_err(|e| AppError::BadRequest(format!("Invalid node id provided. {e:#}")))?;

    backup
        .verify(&state.secp, &node_id)
        .map_err(|_| AppError::Unauthorized)?;

    state
        .user_backup
        .back_up(node_id, backup.0)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))
}

#[instrument(skip_all, err(Debug))]
pub async fn delete_backup(
    Path(node_id): Path<String>,
    State(state): State<Arc<AppState>>,
    backup: Json<DeleteBackup>,
) -> Result<(), AppError> {
    let node_id = PublicKey::from_str(&node_id)
        .map_err(|e| AppError::BadRequest(format!("Invalid node id provided. {e:#}")))?;

    backup
        .verify(&state.secp, &node_id)
        .map_err(|_| AppError::Unauthorized)?;

    state
        .user_backup
        .delete(node_id, backup.0)
        .map_err(|e| AppError::InternalServerError(e.to_string()))
}

#[instrument(skip_all, err(Debug))]
async fn restore(
    Path(node_id): Path<String>,
    State(state): State<Arc<AppState>>,
    signature: Json<Signature>,
) -> Result<Json<Vec<Restore>>, AppError> {
    let node_id = PublicKey::from_str(&node_id)
        .map_err(|e| AppError::BadRequest(format!("Invalid node id provided. {e:#}")))?;

    let message = node_id.to_string().as_bytes().to_vec();
    let message = commons::create_sign_message(message);
    state
        .secp
        .verify_ecdsa(&message, &signature, &node_id)
        .map_err(|_| AppError::Unauthorized)?;

    let backup = state
        .user_backup
        .restore(node_id)
        .map_err(|e| AppError::InternalServerError(format!("Failed to restore backup. {e:#}")))?;

    Ok(Json(backup))
}

fn parse_offset_datetime(date_str: String) -> Result<Option<OffsetDateTime>> {
    if date_str.is_empty() {
        return Ok(None);
    }
    let format = format_description!("[year]-[month]-[day]");
    let date = Date::parse(date_str.as_str(), &format)?;
    let date_time = date.midnight().assume_utc();
    Ok(Some(date_time))
}

#[instrument(skip_all, err(Debug))]
pub async fn get_leaderboard(
    State(state): State<Arc<AppState>>,
    params: Query<LeaderBoardQueryParams>,
) -> Result<Json<LeaderBoard>, AppError> {
    let reverse = params.reverse.unwrap_or_default();
    let top = params.top.unwrap_or(5);

    let start = params.start.clone().unwrap_or_default();
    let start = parse_offset_datetime(start.clone())
        .map_err(|err| {
            AppError::BadRequest(format!(
                "Invalid start date provided `{err}`. String provided {start}"
            ))
        })?
        .unwrap_or(OffsetDateTime::UNIX_EPOCH);

    let end = params.end.clone().unwrap_or_default();
    let end = parse_offset_datetime(end.clone())
        .map_err(|err| {
            AppError::BadRequest(format!(
                "Invalid start date provided `{err}`. String provided {end}"
            ))
        })?
        .unwrap_or(OffsetDateTime::now_utc());

    let category = params.category.clone().unwrap_or(LeaderBoardCategory::Pnl);

    let mut conn = state
        .pool
        .get()
        .map_err(|_| AppError::InternalServerError("Could not access db".to_string()))?;
    let leader_board = generate_leader_board(&mut conn, top, category, reverse, start, end)
        .map_err(|error| {
            AppError::InternalServerError(format!("Could not build leaderboard {error}"))
        })?;

    Ok(Json(LeaderBoard {
        entries: leader_board,
    }))
}

#[instrument(skip_all, err(Debug))]
async fn post_error(
    State(state): State<Arc<AppState>>,
    app_error: Json<ReportedError>,
) -> Result<(), AppError> {
    let mut conn = state
        .pool
        .get()
        .map_err(|e| AppError::InternalServerError(format!("Could not get connection: {e}")))?;

    db::reported_errors::insert(&mut conn, app_error.0)
        .map_err(|e| AppError::InternalServerError(format!("Could not save error in db: {e}")))?;

    Ok(())
}

#[instrument(skip_all, err(Debug))]
async fn create_invoice(
    State(state): State<Arc<AppState>>,
    Json(invoice_params): Json<SignedValue<commons::HodlInvoiceParams>>,
) -> Result<Json<String>, AppError> {
    let public_key = invoice_params.value.trader_pubkey;

    invoice_params
        .verify(&state.secp, &public_key)
        .map_err(|_| AppError::Unauthorized)?;

    let invoice_params = invoice_params.value;
    let invoice_amount = invoice_params.amt_sats;
    let hash = invoice_params.r_hash.clone();

    let mut connection = state
        .pool
        .get()
        .map_err(|_| AppError::InternalServerError("Could not get db connection".to_string()))?;

    let response = state
        .lnd_bridge
        .create_invoice(InvoiceParams {
            value: invoice_amount,
            memo: "Fund your 10101 position".to_string(),
            expiry: 10 * 60, // 10 minutes
            hash: hash.clone(),
        })
        .await
        .map_err(|e| AppError::InternalServerError(format!("{e:#}")))?;

    db::hodl_invoice::create_hodl_invoice(
        &mut connection,
        hash.as_str(),
        public_key,
        invoice_amount,
    )
    .map_err(|error| {
        AppError::InternalServerError(format!("Could not process hodl invoice {error:?}"))
    })?;

    // watch for the created hodl invoice
    invoice::spawn_invoice_watch(
        state.tx_orderbook_feed.clone(),
        state.lnd_bridge.clone(),
        invoice_params,
    );

    tracing::info!(
        trader_pubkey = public_key.to_string(),
        hash,
        amount_sats = invoice_amount,
        "Started watching for hodl invoice"
    );

    Ok(Json(response.payment_request))
}
