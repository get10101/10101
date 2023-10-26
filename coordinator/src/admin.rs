use crate::collaborative_revert;
use crate::db;
use crate::position::models::parse_channel_id;
use crate::routes::AppState;
use crate::AppError;
use anyhow::Context;
use autometrics::autometrics;
use axum::extract::Path;
use axum::extract::Query;
use axum::extract::State;
use axum::Json;
use bdk::TransactionDetails;
use bitcoin::secp256k1::PublicKey;
use bitcoin::OutPoint;
use coordinator_commons::CollaborativeRevert;
use dlc_manager::subchannel::SubChannel;
use lightning_invoice::Invoice;
use ln_dlc_node::node::NodeInfo;
use serde::de;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;
use time::OffsetDateTime;
use tokio::task::spawn_blocking;
use tracing::instrument;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Balance {
    pub offchain: u64,
    pub onchain: u64,
}

#[autometrics]
pub async fn get_balance(State(state): State<Arc<AppState>>) -> Result<Json<Balance>, AppError> {
    spawn_blocking(move || {
        let offchain = state.node.inner.get_ldk_balance();
        let onchain =
            state.node.inner.get_on_chain_balance().map_err(|e| {
                AppError::InternalServerError(format!("Failed to get balance: {e:#}"))
            })?;

        Ok(Json(Balance {
            offchain: offchain.available(),
            onchain: onchain.confirmed,
        }))
    })
    .await
    .map_err(|e| AppError::InternalServerError(format!("Failed to get balance: {e:#}")))?
}

#[derive(Serialize)]
pub struct ChannelDetails {
    #[serde(flatten)]
    pub channel_details: ln_dlc_node::ChannelDetails,
    pub user_email: String,
}

impl From<(lightning::ln::channelmanager::ChannelDetails, String)> for ChannelDetails {
    fn from(
        (channel_details, user_email): (lightning::ln::channelmanager::ChannelDetails, String),
    ) -> Self {
        ChannelDetails {
            channel_details: ln_dlc_node::ChannelDetails::from(channel_details),
            user_email,
        }
    }
}

pub async fn list_channels(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<ChannelDetails>>, AppError> {
    let mut conn =
        state.pool.clone().get().map_err(|e| {
            AppError::InternalServerError(format!("Failed to acquire db lock: {e:#}"))
        })?;

    let channels = state
        .node
        .inner
        .list_channels()
        .into_iter()
        .map(|channel| {
            let user_email =
                match db::user::by_id(&mut conn, channel.counterparty.node_id.to_string()) {
                    Ok(Some(user)) => user.email,
                    _ => "unknown".to_string(),
                };
            ChannelDetails::from((channel, user_email))
        })
        .collect::<Vec<_>>();

    Ok(Json(channels))
}

#[derive(Serialize)]
pub struct DlcChannelDetails {
    #[serde(flatten)]
    pub channel_details: ln_dlc_node::DlcChannelDetails,
    pub user_email: String,
    #[serde(with = "time::serde::rfc3339::option")]
    pub user_registration_timestamp: Option<OffsetDateTime>,
}

impl From<(SubChannel, String, Option<OffsetDateTime>)> for DlcChannelDetails {
    fn from(
        (channel_details, user_email, user_registration_timestamp): (
            SubChannel,
            String,
            Option<OffsetDateTime>,
        ),
    ) -> Self {
        DlcChannelDetails {
            channel_details: ln_dlc_node::DlcChannelDetails::from(channel_details),
            user_email,
            user_registration_timestamp,
        }
    }
}

#[autometrics]
pub async fn list_dlc_channels(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<DlcChannelDetails>>, AppError> {
    let mut conn =
        state.pool.clone().get().map_err(|e| {
            AppError::InternalServerError(format!("Failed to acquire db lock: {e:#}"))
        })?;

    let dlc_channels = state.node.inner.list_dlc_channels().map_err(|e| {
        AppError::InternalServerError(format!("Failed to list DLC channels: {e:#}"))
    })?;

    let dlc_channels = dlc_channels
        .into_iter()
        .map(|subchannel| {
            let (email, registration_timestamp) =
                match db::user::by_id(&mut conn, subchannel.counter_party.to_string()) {
                    Ok(Some(user)) => (user.email, Some(user.timestamp)),
                    _ => ("unknown".to_string(), None),
                };
            DlcChannelDetails::from((subchannel, email, registration_timestamp))
        })
        .collect::<Vec<_>>();

    Ok(Json(dlc_channels))
}

pub async fn collaborative_revert(
    State(state): State<Arc<AppState>>,
    revert_params: Json<CollaborativeRevert>,
) -> Result<Json<String>, AppError> {
    let channel_id_string = revert_params.channel_id.clone();
    let channel_id = parse_channel_id(channel_id_string.as_str()).map_err(|error| {
        tracing::error!(
            channel_id = channel_id_string,
            "Invalid channel id provided. {error:#}"
        );
        AppError::BadRequest("Invalid channel id provided".to_string())
    })?;

    let out_point = OutPoint {
        txid: revert_params.txid,
        vout: revert_params.vout,
    };
    collaborative_revert::notify_user_to_collaboratively_revert(
        revert_params,
        channel_id_string.clone(),
        channel_id,
        state.pool.clone(),
        state.node.inner.clone(),
        state.auth_users_notifier.clone(),
        out_point,
    )
    .await
    .map_err(move |error| {
        tracing::error!(
            channel_id = channel_id_string,
            "Could not collaborativel revert channel. {error:#}"
        );
        AppError::InternalServerError("Could not collaboratively revert channel".to_string())
    })?;

    Ok(Json(
        "Successfully notified trader, waiting for him to ping us again".to_string(),
    ))
}

pub async fn list_on_chain_transactions(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<TransactionDetails>>, AppError> {
    spawn_blocking(move || {
        let transactions = state.node.inner.get_on_chain_history().map_err(|e| {
            AppError::InternalServerError(format!("Failed to list transactions: {e:#}"))
        })?;
        Ok(Json(transactions))
    })
    .await
    .map_err(|e| AppError::InternalServerError(format!("Failed to list transactions: {e:#}")))?
}

pub async fn list_peers(State(state): State<Arc<AppState>>) -> Json<Vec<PublicKey>> {
    let peers = state.node.inner.list_peers();
    Json(peers)
}

#[derive(Debug, Deserialize)]
pub struct CloseChannelParams {
    #[serde(default, deserialize_with = "empty_string_as_none")]
    force: Option<bool>,
}

fn empty_string_as_none<'de, D, T>(de: D) -> Result<Option<T>, D::Error>
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

#[derive(Deserialize)]
pub struct ChannelParams {
    target: TargetInfo,
    local_balance: u64,
    remote_balance: Option<u64>,
}

#[derive(Deserialize)]
pub struct TargetInfo {
    pubkey: String,
    address: Option<String>,
}

pub async fn open_channel(
    State(state): State<Arc<AppState>>,
    channel_params: Json<ChannelParams>,
) -> Result<Json<String>, AppError> {
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

    let channel_id = state
        .node
        .inner
        .initiate_open_channel(pubkey, channel_amount, initial_send_amount, true)
        .map_err(|e| AppError::InternalServerError(format!("Failed to open channel: {e:#}")))?;

    tracing::debug!(
        "Successfully opened channel with {pubkey}. Funding tx: {}",
        hex::encode(channel_id)
    );

    Ok(Json(hex::encode(channel_id)))
}

pub async fn send_payment(
    Path(invoice): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<(), AppError> {
    let invoice = Invoice::from_str(invoice.as_str())
        .context("Could not parse Invoice string")
        .map_err(|e| AppError::BadRequest(format!("{e:#}")))?;
    state
        .node
        .inner
        .pay_invoice(&invoice, None)
        .map_err(|e| AppError::InternalServerError(format!("{e:#}")))?;

    Ok(())
}

#[instrument(skip_all, err(Debug))]
pub async fn close_channel(
    Path(channel_id_string): Path<String>,
    Query(params): Query<CloseChannelParams>,
    State(state): State<Arc<AppState>>,
) -> Result<(), AppError> {
    let channel_id = hex::decode(channel_id_string.clone())
        .map_err(|err| AppError::BadRequest(err.to_string()))?;

    let channel_id: [u8; 32] = channel_id
        .try_into()
        .map_err(|_| AppError::BadRequest("Provided channel ID was invalid".to_string()))?;

    tracing::info!(channel_id = %channel_id_string, "Attempting to close channel");

    state
        .node
        .inner
        .close_channel(channel_id, params.force.unwrap_or_default())
        .map_err(|e| AppError::InternalServerError(format!("{e:#}")))?;

    Ok(())
}

pub async fn sign_message(
    Path(msg): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<String>, AppError> {
    let signature =
        state.node.inner.sign_message(msg).map_err(|err| {
            AppError::InternalServerError(format!("Could not sign message {err}"))
        })?;

    Ok(Json(signature))
}

pub async fn connect_to_peer(
    State(state): State<Arc<AppState>>,
    target: Json<NodeInfo>,
) -> Result<(), AppError> {
    let target = target.0;
    state.node.inner.connect(target).await.map_err(|err| {
        AppError::InternalServerError(format!("Could not connect to {target}. Error: {err}"))
    })?;
    Ok(())
}

pub async fn is_connected(
    State(state): State<Arc<AppState>>,
    Path(target_pubkey): Path<String>,
) -> Result<Json<bool>, AppError> {
    let target = target_pubkey.parse().map_err(|err| {
        AppError::BadRequest(format!("Invalid public key {target_pubkey}. Error: {err}"))
    })?;
    Ok(Json(state.node.is_connected(&target)))
}
