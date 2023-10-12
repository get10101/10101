use crate::admin::close_channel;
use crate::admin::collaborative_revert;
use crate::admin::connect_to_peer;
use crate::admin::get_balance;
use crate::admin::is_connected;
use crate::admin::list_channels;
use crate::admin::list_dlc_channels;
use crate::admin::list_on_chain_transactions;
use crate::admin::list_peers;
use crate::admin::open_channel;
use crate::admin::send_payment;
use crate::admin::sign_message;
use crate::db;
use crate::db::liquidity::LiquidityRequestLog;
use crate::db::positions::Position;
use crate::db::user;
use crate::is_liquidity_sufficient;
use crate::message::NewUserMessage;
use crate::message::OrderbookMessage;
use crate::node::Node;
use crate::orderbook::routes::get_order;
use crate::orderbook::routes::get_orders;
use crate::orderbook::routes::post_order;
use crate::orderbook::routes::put_order;
use crate::orderbook::routes::websocket_handler;
use crate::orderbook::trading::NewOrderMessage;
use crate::settings::Settings;
use crate::AppError;
use anyhow::Context;
use autometrics::autometrics;
use axum::extract::Path;
use axum::extract::Query;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::delete;
use axum::routing::get;
use axum::routing::post;
use axum::Json;
use axum::Router;
use bitcoin::consensus::encode::serialize_hex;
use bitcoin::hashes::hex::ToHex;
use bitcoin::secp256k1::PublicKey;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::Network;
use coordinator_commons::CollaborativeRevertData;
use coordinator_commons::LspConfig;
use coordinator_commons::OnboardingParam;
use coordinator_commons::RegisterParams;
use coordinator_commons::TradeParams;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::PgConnection;
use dlc_manager::subchannel::LNChannelManager;
use dlc_manager::ChannelId;
use hex::FromHex;
use lightning::ln::msgs::NetAddress;
use lightning::util::errors::APIError;
use ln_dlc_node::channel::UserChannelId;
use ln_dlc_node::node::peer_manager::alias_as_bytes;
use ln_dlc_node::node::peer_manager::broadcast_node_announcement;
use ln_dlc_node::node::LiquidityRequest;
use ln_dlc_node::node::NodeInfo;
use opentelemetry_prometheus::PrometheusExporter;
use orderbook_commons::Message;
use orderbook_commons::RouteHintHop;
use prometheus::Encoder;
use prometheus::TextEncoder;
use serde::Deserialize;
use serde::Serialize;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::sync::RwLock;
use tokio::task::spawn_blocking;
use tracing::instrument;

pub struct AppState {
    pub node: Node,
    // Channel used to send messages to all connected clients.
    pub tx_price_feed: broadcast::Sender<Message>,
    pub tx_user_feed: broadcast::Sender<NewUserMessage>,
    pub trading_sender: mpsc::Sender<NewOrderMessage>,
    pub pool: Pool<ConnectionManager<PgConnection>>,
    pub settings: RwLock<Settings>,
    pub exporter: PrometheusExporter,
    pub announcement_addresses: Vec<NetAddress>,
    pub node_alias: String,
    pub auth_users_notifier: mpsc::Sender<OrderbookMessage>,
}

#[allow(clippy::too_many_arguments)]
pub fn router(
    node: Node,
    pool: Pool<ConnectionManager<PgConnection>>,
    settings: Settings,
    exporter: PrometheusExporter,
    announcement_addresses: Vec<NetAddress>,
    node_alias: &str,
    trading_sender: mpsc::Sender<NewOrderMessage>,
    tx_price_feed: broadcast::Sender<Message>,
    tx_user_feed: broadcast::Sender<NewUserMessage>,
    auth_users_notifier: mpsc::Sender<OrderbookMessage>,
) -> Router {
    let app_state = Arc::new(AppState {
        node,
        pool,
        settings: RwLock::new(settings),
        tx_price_feed,
        tx_user_feed,
        trading_sender,
        exporter,
        announcement_addresses,
        node_alias: node_alias.to_string(),
        auth_users_notifier,
    });

    Router::new()
        .route("/", get(index))
        .route("/api/version", get(version))
        .route(
            "/api/prepare_onboarding_payment",
            post(prepare_onboarding_payment),
        )
        .route(
            "/api/prepare_regular_payment/:peer_id",
            post(prepare_regular_payment),
        )
        .route("/api/newaddress", get(get_unused_address))
        .route("/api/node", get(get_node_info))
        .route("/api/invoice", get(get_invoice))
        .route(
            "/api/invoice/open_channel_fee",
            get(get_open_channel_fee_invoice),
        )
        .route("/api/orderbook/orders", get(get_orders).post(post_order))
        .route(
            "/api/orderbook/orders/:order_id",
            get(get_order).put(put_order),
        )
        .route("/api/orderbook/websocket", get(websocket_handler))
        .route("/api/trade", post(post_trade))
        .route("/api/rollover/:dlc_channel_id", post(rollover))
        .route("/api/register", post(post_register))
        .route("/api/admin/balance", get(get_balance))
        .route("/api/admin/channels", get(list_channels).post(open_channel))
        .route("/api/channels", post(channel_faucet))
        .route("/api/lsp/config", get(get_lsp_channel_config))
        .route("/api/admin/channels/:channel_id", delete(close_channel))
        .route("/api/admin/peers", get(list_peers))
        .route("/api/admin/send_payment/:invoice", post(send_payment))
        .route("/api/admin/dlc_channels", get(list_dlc_channels))
        .route("/api/admin/transactions", get(list_on_chain_transactions))
        .route("/api/admin/sign/:msg", get(sign_message))
        .route("/api/admin/connect", post(connect_to_peer))
        .route("/api/admin/channels/revert", post(collaborative_revert))
        .route(
            "/api/channels/revertconfirm",
            post(collaborative_revert_confirm),
        )
        .route("/api/admin/is_connected/:target_pubkey", get(is_connected))
        .route(
            "/api/admin/settings",
            get(get_settings).put(update_settings),
        )
        .route("/api/admin/sync", post(post_sync))
        .route(
            "/api/admin/broadcast_announcement",
            post(post_broadcast_announcement),
        )
        .route("/metrics", get(get_metrics))
        .route("/health", get(get_health))
        .with_state(app_state)
}

#[derive(serde::Serialize)]
struct HelloWorld {
    hello: String,
}

pub async fn index() -> impl IntoResponse {
    Json(HelloWorld {
        hello: "world".to_string(),
    })
}

pub async fn prepare_onboarding_payment(
    State(app_state): State<Arc<AppState>>,
    params: Json<OnboardingParam>,
) -> Result<Json<RouteHintHop>, AppError> {
    let Json(OnboardingParam {
        target_node,
        user_channel_id,
        amount_sats,
        liquidity_option_id,
    }) = params;

    let target_node: PublicKey = target_node.parse().map_err(|e| {
        AppError::BadRequest(format!(
            "Provided public key {target_node} was not valid: {e:#}"
        ))
    })?;

    let user_channel_id = UserChannelId::try_from(user_channel_id.clone()).map_err(|e| {
        AppError::BadRequest(format!(
            "Provided user channel id {user_channel_id} was not valid: {e:#}"
        ))
    })?;

    let mut conn = app_state
        .pool
        .get()
        .map_err(|e| AppError::InternalServerError(format!("Could not get connection: {e:#}")))?;

    let balance = app_state
        .node
        .inner
        .get_on_chain_balance()
        .map_err(|e| AppError::InternalServerError(format!("Could not get balance: {e:#}")))?;

    let have_enough_liquidity =
        is_liquidity_sufficient(&*app_state.settings.read().await, balance, amount_sats);

    LiquidityRequestLog::insert(
        &mut conn,
        target_node,
        amount_sats,
        liquidity_option_id,
        have_enough_liquidity,
    )
    .map_err(|e| {
        AppError::InternalServerError(format!("Could not insert liquidity request: {e:#}"))
    })?;

    if !have_enough_liquidity {
        return Err(AppError::ServiceUnavailable(
            "Coordinator cannot provide required liquidity".to_string(),
        ));
    };

    let route_hint_hop = spawn_blocking({
        let app_state = app_state.clone();
        move || {
            let mut conn = app_state.pool.get()?;
            let liquidity_option = db::liquidity_options::get(&mut conn, liquidity_option_id)?;
            app_state
                .node
                .inner
                .prepare_onboarding_payment(LiquidityRequest {
                    user_channel_id,
                    liquidity_option_id,
                    trader_id: target_node,
                    trade_up_to_sats: liquidity_option.trade_up_to_sats,
                    max_deposit_sats: liquidity_option.max_deposit_sats,
                    coordinator_leverage: liquidity_option.coordinator_leverage,
                })
        }
    })
    .await
    .expect("task to complete")
    .map_err(|e| AppError::InternalServerError(format!("Could not prepare payment: {e:#}")))?;

    Ok(Json(route_hint_hop.into()))
}

pub async fn prepare_regular_payment(
    peer_id: Path<String>,
    State(app_state): State<Arc<AppState>>,
) -> Result<Json<RouteHintHop>, AppError> {
    let peer_id = peer_id.0;
    let peer_id: PublicKey = peer_id.parse().map_err(|e| {
        AppError::BadRequest(format!(
            "Provided public key {peer_id} was not valid: {e:#}"
        ))
    })?;

    let route_hint_hop = spawn_blocking({
        let app_state = app_state.clone();
        move || {
            app_state
                .node
                .inner
                .prepare_payment_with_route_hint(peer_id)
        }
    })
    .await
    .expect("task to complete")
    .map_err(|e| AppError::InternalServerError(format!("Could not prepare payment: {e:#}")))?;

    Ok(Json(route_hint_hop.into()))
}

pub async fn get_unused_address(State(app_state): State<Arc<AppState>>) -> impl IntoResponse {
    app_state.node.inner.get_unused_address().to_string()
}

pub async fn get_node_info(
    State(app_state): State<Arc<AppState>>,
) -> Result<Json<NodeInfo>, AppError> {
    let node_info = app_state.node.inner.info;
    Ok(Json(node_info))
}

#[derive(Debug, Deserialize, Serialize)]
pub struct InvoiceParams {
    pub amount: Option<u64>,
    pub description: Option<String>,
    pub expiry: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct OpenChannelFeeInvoiceParams {
    pub amount: u64,
    pub channel_funding_txid: String,
    pub expiry: Option<u32>,
}

pub async fn get_invoice(
    Query(params): Query<InvoiceParams>,
    State(state): State<Arc<AppState>>,
) -> Result<String, AppError> {
    let invoice = state
        .node
        .inner
        .create_invoice(
            params.amount.unwrap_or_default(),
            params.description.unwrap_or_default(),
            params.expiry.unwrap_or(180),
        )
        .map_err(|e| AppError::InternalServerError(format!("Failed to create invoice: {e:#}")))?;

    Ok(invoice.to_string())
}

pub async fn get_open_channel_fee_invoice(
    Query(params): Query<OpenChannelFeeInvoiceParams>,
    State(state): State<Arc<AppState>>,
) -> Result<String, AppError> {
    let invoice = state
        .node
        .channel_opening_fee_invoice(params.amount, params.channel_funding_txid, params.expiry)
        .await
        .map_err(|e| AppError::InternalServerError(format!("Failed to create invoice: {e:#}")))?;

    Ok(invoice.to_string())
}

// TODO: We might want to have our own ContractInput type here so we can potentially map fields if
// the library changes?
#[instrument(skip_all, err(Debug))]
#[autometrics]
pub async fn post_trade(
    State(state): State<Arc<AppState>>,
    trade_params: Json<TradeParams>,
) -> Result<String, AppError> {
    let invoice = state.node.trade(&trade_params.0).await.map_err(|e| {
        AppError::InternalServerError(format!("Could not handle trade request: {e:#}"))
    })?;

    Ok(invoice.to_string())
}

#[instrument(skip_all, err(Debug))]
#[autometrics]
pub async fn rollover(
    State(state): State<Arc<AppState>>,
    Path(dlc_channel_id): Path<String>,
) -> Result<(), AppError> {
    let dlc_channel_id = ChannelId::from_hex(dlc_channel_id.clone()).map_err(|e| {
        AppError::InternalServerError(format!(
            "Could not decode dlc channel id from {dlc_channel_id}: {e:#}"
        ))
    })?;

    state
        .node
        .propose_rollover(dlc_channel_id, state.node.inner.network)
        .await
        .map_err(|e| {
            AppError::InternalServerError(format!(
                "Failed to rollover dlc channel with id {}: {e:#}",
                dlc_channel_id.to_hex()
            ))
        })?;

    Ok(())
}

pub async fn post_broadcast_announcement(
    State(state): State<Arc<AppState>>,
) -> Result<(), AppError> {
    let node_alias = alias_as_bytes(state.node_alias.as_str()).map_err(|e| {
        AppError::InternalServerError(format!(
            "Could not parse node alias {0} due to {e:#}",
            state.node_alias
        ))
    })?;
    broadcast_node_announcement(
        &state.node.inner.peer_manager,
        node_alias,
        state.announcement_addresses.clone(),
    );

    Ok(())
}

/// Internal API for syncing the wallet
#[instrument(skip_all, err(Debug))]
#[autometrics]
pub async fn post_sync(State(state): State<Arc<AppState>>) -> Result<(), AppError> {
    spawn_blocking(move || state.node.inner.wallet().sync())
        .await
        .map_err(|_| AppError::InternalServerError("Could not sync wallet".to_string()))?
        .map_err(|e| AppError::InternalServerError(format!("Could not sync wallet: {e:#}")))?;

    Ok(())
}

#[instrument(skip_all, err(Debug))]
pub async fn post_register(
    State(state): State<Arc<AppState>>,
    params: Json<RegisterParams>,
) -> Result<(), AppError> {
    let register_params = params.0;
    if !register_params.is_valid() {
        return Err(AppError::BadRequest(format!(
            "Invalid registration parameters: {register_params:?}"
        )));
    }
    tracing::info!(?register_params, "Registered new user");

    let mut conn = state
        .pool
        .get()
        .map_err(|e| AppError::InternalServerError(format!("Could not get connection: {e:#}")))?;

    if let Some(email) = register_params.email {
        user::upsert_email(&mut conn, register_params.pubkey, email)
            .map_err(|e| AppError::InternalServerError(format!("Could not upsert user: {e:#}")))?;
    } else {
        tracing::warn!(trader_id=%register_params.pubkey, "Did not receive an email during registration");
    }

    Ok(())
}

pub async fn get_lsp_channel_config(
    State(state): State<Arc<AppState>>,
) -> Result<Json<LspConfig>, AppError> {
    let mut conn = state
        .pool
        .get()
        .map_err(|e| AppError::InternalServerError(format!("Could not get connection: {e:#}")))?;

    let liquidity_options = db::liquidity_options::get_all(&mut conn).map_err(|e| {
        AppError::InternalServerError(format!("Failed to get all liquidity options: {e:#}"))
    })?;

    let settings = state.settings.read().await;
    Ok(Json(LspConfig {
        contract_tx_fee_rate: settings.contract_tx_fee_rate,
        liquidity_options,
    }))
}

/// Open a channel directly between the coordinator and the target
/// specified in [`ChannelParams`].
///
/// Can only be used on [`Network::Regtest`].
pub async fn channel_faucet(
    State(state): State<Arc<AppState>>,
    channel_params: Json<ChannelParams>,
) -> Result<Json<String>, AppError> {
    let network = state.node.inner.network;
    if network != Network::Regtest {
        return Err(AppError::BadRequest(format!(
            "Cannot open channel on {network}"
        )));
    }

    if !state.settings.read().await.jit_channels_enabled {
        return Err(AppError::BadRequest(
            "JIT channels are not enabled".to_string(),
        ));
    }

    let pubkey = PublicKey::from_str(channel_params.0.target.pubkey.as_str())
        .map_err(|e| AppError::BadRequest(format!("Invalid target node pubkey provided {e:#}")))?;
    if let Some(address) = channel_params.target.address.clone() {
        let target_address = address.parse().map_err(|e| {
            AppError::BadRequest(format!("Invalid target node address provided {e:#}"))
        })?;
        let peer = NodeInfo {
            pubkey,
            address: target_address,
        };
        state.node.inner.connect(peer).await.map_err(|e| {
            AppError::InternalServerError(format!("Could not connect to target node {e:#}"))
        })?;
    }

    let channel_amount = channel_params.local_balance;
    let initial_send_amount = channel_params.remote_balance.unwrap_or_default();
    let is_public = channel_params.is_public;

    let channel_id = state
        .node
        .inner
        .initiate_open_channel(pubkey, channel_amount, initial_send_amount, is_public)
        .map_err(|e| AppError::InternalServerError(format!("Failed to open channel: {e:#}")))?;

    tracing::debug!(
        "Successfully opened channel with {pubkey}. Funding tx: {}",
        hex::encode(channel_id)
    );

    Ok(Json(hex::encode(channel_id)))
}

#[derive(Deserialize)]
pub struct ChannelParams {
    target: TargetInfo,
    local_balance: u64,
    remote_balance: Option<u64>,
    is_public: bool,
}

#[derive(Deserialize)]
pub struct TargetInfo {
    pubkey: String,
    address: Option<String>,
}

async fn get_settings(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let settings = state.settings.read().await;
    serde_json::to_string(&*settings).expect("to be able to serialise settings")
}

async fn update_settings(
    State(state): State<Arc<AppState>>,
    Json(updated_settings): Json<Settings>,
) -> Result<(), AppError> {
    // Update settings in memory
    *state.settings.write().await = updated_settings.clone();

    updated_settings
        .write_to_file()
        .await
        .map_err(|e| AppError::InternalServerError(format!("Could not write settings: {e:#}")))?;

    // Forward relevant settings down to the node
    state
        .node
        .update_settings(updated_settings.to_node_settings())
        .await;

    // Forward relevant settings down to the wallet
    state
        .node
        .inner
        .update_settings(updated_settings.ln_dlc.clone())
        .await;

    state
        .node
        .update_ldk_settings(updated_settings.to_ldk_settings());

    Ok(())
}

pub async fn get_metrics(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let autometrics = match autometrics::prometheus_exporter::encode_to_string() {
        Ok(metrics) => metrics,
        Err(err) => {
            tracing::error!("Could not collect autometrics {err:#}");
            return (StatusCode::INTERNAL_SERVER_ERROR, format!("{:?}", err));
        }
    };

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

    (StatusCode::OK, open_telemetry_metrics + &autometrics)
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

pub async fn collaborative_revert_confirm(
    State(state): State<Arc<AppState>>,
    revert_params: Json<CollaborativeRevertData>,
) -> Result<Json<String>, AppError> {
    let mut conn = state.pool.clone().get().map_err(|error| {
        AppError::InternalServerError(format!("Could not acquire db lock {error:#}"))
    })?;

    let channel_id_string = revert_params.channel_id.clone();
    let channel_id = hex::decode(channel_id_string.clone())
        .map_err(|err| AppError::BadRequest(err.to_string()))?;
    let channel_id: [u8; 32] = channel_id
        .try_into()
        .map_err(|_| AppError::BadRequest("Provided channel ID was invalid".to_string()))?;

    tracing::info!(
        channel_id = channel_id_string,
        "Confirming collaborative channel revert"
    );
    let inner_node = state.node.inner.clone();

    // TODO: check if provided amounts are as expected
    if !revert_params
        .transaction
        .output
        .iter()
        .any(|output| inner_node.wallet().is_mine(&output.script_pubkey).is_ok())
    {
        let error_message = "Invalid request: no address for coordinator provided".to_string();
        tracing::error!(error_message);
        return Err(AppError::BadRequest(error_message));
    }

    let sub_channels = inner_node.list_dlc_channels().map_err(|e| {
        AppError::InternalServerError(format!("Failed to list DLC channels: {e:#}"))
    })?;
    let subchannel = sub_channels
        .iter()
        .find(|c| c.channel_id == channel_id)
        .context("Could not find provided channel")
        .map_err(|e| AppError::BadRequest(format!("Channel not found: {e:#}")))?;

    let channel_manager = inner_node.channel_manager.clone();

    let mut own_sig = None;

    let mut revert_transaction = revert_params.transaction.clone();

    let position = Position::get_position_by_channel_id(&mut conn, channel_id_string.clone())
        .map_err(|error| {
            tracing::error!(
                channel_id = channel_id_string,
                "Could not get position by channel_id"
            );
            AppError::InternalServerError(format!(
                "Could not get open position by channel id {channel_id_string}. {error:#}"
            ))
        })?;

    channel_manager
        .with_channel_lock_no_check(
            &subchannel.channel_id,
            &subchannel.counter_party,
            |channel_lock| {
                channel_manager.sign_with_fund_key_cb(channel_lock, &mut |fund_sk| {
                    let secp = Secp256k1::new();

                    own_sig = Some(
                        dlc::util::get_raw_sig_for_tx_input(
                            &secp,
                            &revert_transaction,
                            0,
                            &subchannel.original_funding_redeemscript,
                            subchannel.fund_value_satoshis,
                            fund_sk,
                        )
                        .expect("To be able to get raw sig for tx inpout"),
                    );

                    dlc::util::sign_multi_sig_input(
                        &secp,
                        &mut revert_transaction,
                        &revert_params.signature,
                        &subchannel.counter_fund_pk,
                        fund_sk,
                        &subchannel.original_funding_redeemscript,
                        subchannel.fund_value_satoshis,
                        0,
                    )
                    .expect("To be able to sign multi sig");
                });
                Ok(())
            },
        )
        .map_err(|error| {
            let error = match error {
                APIError::APIMisuseError { .. } => "APIMisuseError",
                APIError::FeeRateTooHigh { .. } => "FeeRateTooHigh",
                APIError::InvalidRoute { .. } => "InvalidRoute",
                APIError::ChannelUnavailable { .. } => "ChannelUnavailable",
                APIError::MonitorUpdateInProgress => "MonitorUpdateInProgress",
                APIError::IncompatibleShutdownScript { .. } => "IncompatibleShutdownScript",
                APIError::ExternalError { .. } => "ExternalError",
            };
            AppError::InternalServerError(format!("Could not sign revert transaction {error:#}"))
        })?;

    // if we have a sig here, it means we were able to sign the transaction and can broadcast it
    if own_sig.is_some() {
        let string = serialize_hex(&revert_transaction);
        inner_node
            .wallet()
            .broadcast_transaction(&revert_transaction)
            .map_err(|error| {
                AppError::InternalServerError(format!("Could not broadcast transaction {error:#}"))
            })?;

        Position::set_position_to_closed(&mut conn, position.id).map_err(|error| {
            AppError::InternalServerError(format!("Could set position to closed {error:#}"))
        })?;

        Ok(Json(string))
    } else {
        Err(AppError::InternalServerError(
            "Failed to sign revert transaction ".to_string(),
        ))
    }
}
