use crate::admin::close_channel;
use crate::admin::collaborative_revert;
use crate::admin::connect_to_peer;
use crate::admin::delete_dlc_channel;
use crate::admin::get_balance;
use crate::admin::get_fee_rate_estimation;
use crate::admin::get_utxos;
use crate::admin::is_connected;
use crate::admin::list_dlc_channels;
use crate::admin::list_on_chain_transactions;
use crate::admin::list_peers;
use crate::admin::migrate_dlc_channels;
use crate::admin::resend_renew_revoke_message;
use crate::admin::roll_back_dlc_channel;
use crate::admin::rollover;
use crate::admin::sign_message;
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
use crate::node::Node;
use crate::notifications::Notification;
use crate::orderbook::routes::delete_order;
use crate::orderbook::routes::get_order;
use crate::orderbook::routes::get_orders;
use crate::orderbook::routes::post_order;
use crate::orderbook::routes::websocket_handler;
use crate::orderbook::trading::NewOrderMessage;
use crate::parse_dlc_channel_id;
use crate::referrals;
use crate::settings::Settings;
use crate::settings::SettingsFile;
use crate::trade::websocket::InternalPositionUpdateMessage;
use crate::AppError;
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
use commons::Backup;
use commons::CollaborativeRevertTraderResponse;
use commons::DeleteBackup;
use commons::Message;
use commons::Poll;
use commons::PollAnswers;
use commons::RegisterParams;
use commons::Restore;
use commons::UpdateUsernameParams;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::PgConnection;
use lightning::ln::msgs::SocketAddress;
use ln_dlc_node::node::NodeInfo;
use opentelemetry_prometheus::PrometheusExporter;
use prometheus::Encoder;
use prometheus::TextEncoder;
use serde::de;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use std::fmt;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use time::macros::format_description;
use time::Date;
use time::OffsetDateTime;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::sync::RwLock;
use tokio::task::spawn_blocking;
use tracing::instrument;

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
    pub announcement_addresses: Vec<SocketAddress>,
    pub node_alias: String,
    pub auth_users_notifier: mpsc::Sender<OrderbookMessage>,
    pub notification_sender: mpsc::Sender<Notification>,
    pub user_backup: SledBackup,
    pub secp: Secp256k1<VerifyOnly>,
}

#[allow(clippy::too_many_arguments)]
pub fn router(
    node: Node,
    pool: Pool<ConnectionManager<PgConnection>>,
    settings: Settings,
    exporter: PrometheusExporter,
    announcement_addresses: Vec<SocketAddress>,
    node_alias: &str,
    trading_sender: mpsc::Sender<NewOrderMessage>,
    tx_orderbook_feed: broadcast::Sender<Message>,
    tx_position_feed: broadcast::Sender<InternalPositionUpdateMessage>,
    tx_user_feed: broadcast::Sender<NewUserMessage>,
    auth_users_notifier: mpsc::Sender<OrderbookMessage>,
    notification_sender: mpsc::Sender<Notification>,
    user_backup: SledBackup,
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
        announcement_addresses,
        node_alias: node_alias.to_string(),
        auth_users_notifier,
        notification_sender,
        user_backup,
        secp,
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
        // Deprecated: we just keep it for backwards compatbility as otherwise old apps won't
        // pass registration
        .route("/api/register", post(post_register))
        .route("/api/users", post(post_register))
        .route("/api/users/:trader_pubkey", get(get_user))
        .route("/api/users/nickname", put(update_nickname))
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
        .route("/api/admin/sign/:msg", get(sign_message))
        .route("/api/admin/connect", post(connect_to_peer))
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
                ln_dlc_node::networking::axum::setup_inbound(peer_manager, socket, addr)
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

#[derive(Debug, Deserialize)]
pub struct SyncParams {
    #[serde(default, deserialize_with = "empty_string_as_none")]
    full: Option<bool>,
    #[serde(default, deserialize_with = "empty_string_as_none")]
    gap: Option<usize>,
}

/// Internal API for syncing the on-chain wallet and the DLC channels.
#[instrument(skip_all, err(Debug))]
pub async fn post_sync(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SyncParams>,
) -> Result<(), AppError> {
    if params.full.unwrap_or(false) {
        let stop_gap = params.gap.unwrap_or(20);

        state.node.inner.full_sync(stop_gap).await.map_err(|e| {
            AppError::InternalServerError(format!("Could not full-sync on-chain wallet: {e:#}"))
        })?;
    } else {
        state.node.inner.sync_on_chain_wallet().await.map_err(|e| {
            AppError::InternalServerError(format!("Could not sync on-chain wallet: {e:#}"))
        })?;
    }

    spawn_blocking(move || {
        if let Err(e) = state.node.inner.dlc_manager.periodic_check() {
            tracing::error!("Failed to run DLC manager periodic check: {e:#}");
        };
    })
    .await
    .expect("task to complete");

    Ok(())
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

#[instrument(skip_all, err(Debug))]
pub async fn get_user_referral_status(
    State(state): State<Arc<AppState>>,
    Path(trader_pubkey): Path<String>,
) -> Result<Json<commons::ReferralStatus>, AppError> {
    let mut conn = state
        .pool
        .get()
        .map_err(|e| AppError::InternalServerError(format!("Could not get connection: {e:#}")))?;

    let trader_pubkey = PublicKey::from_str(trader_pubkey.as_str())
        .map_err(|_| AppError::BadRequest("Invalid trader id provided".to_string()))?;

    let referral_status =
        referrals::get_referral_status(trader_pubkey, &mut conn).map_err(|err| {
            AppError::InternalServerError(format!("Could not calculate referral state {err:?}"))
        })?;
    Ok(Json(referral_status))
}

async fn get_settings(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let settings = state.settings.read().await;
    serde_json::to_string(&*settings).expect("to be able to serialise settings")
}

#[instrument(skip_all, err(Debug))]
async fn update_settings(
    State(state): State<Arc<AppState>>,
    Json(updated_settings): Json<SettingsFile>,
) -> Result<(), AppError> {
    let mut settings = state.settings.write().await;

    settings.update(updated_settings.clone());

    settings
        .write_to_file()
        .await
        .map_err(|e| AppError::InternalServerError(format!("Could not write settings: {e:#}")))?;

    // Forward relevant settings down to the LN-DLC node.
    state
        .node
        .inner
        .update_settings(settings.ln_dlc.clone())
        .await;

    Ok(())
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

fn parse_offset_datetime(date_str: String) -> anyhow::Result<Option<OffsetDateTime>> {
    if date_str.is_empty() {
        return Ok(None);
    }
    let format = format_description!("[year]-[month]-[day]");
    let date = Date::parse(date_str.as_str(), &format)?;
    let date_time = date.midnight().assume_utc();
    Ok(Some(date_time))
}

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

pub fn empty_string_as_none<'de, D, T>(de: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: FromStr,
    T::Err: fmt::Display,
{
    let opt = Option::<String>::deserialize(de)?;
    match opt.as_deref() {
        None | Some("") => Ok(None),
        Some(s) => FromStr::from_str(s).map_err(de::Error::custom).map(Some),
    }
}
