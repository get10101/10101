use crate::db;
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
use tokio::task::spawn_blocking;
use tracing::instrument;

#[derive(Serialize, Deserialize)]
pub struct Balance {
    offchain: u64,
    onchain: u64,
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
            offchain: offchain.available,
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

#[autometrics]
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
}

impl From<(SubChannel, String)> for DlcChannelDetails {
    fn from((channel_details, user_email): (SubChannel, String)) -> Self {
        DlcChannelDetails {
            channel_details: ln_dlc_node::DlcChannelDetails::from(channel_details),
            user_email,
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
            let user_email = match db::user::by_id(&mut conn, subchannel.counter_party.to_string())
            {
                Ok(Some(user)) => user.email,
                _ => "unknown".to_string(),
            };
            DlcChannelDetails::from((subchannel, user_email))
        })
        .collect::<Vec<_>>();

    Ok(Json(dlc_channels))
}

pub async fn list_on_chain_transactions(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<TransactionDetails>>, AppError> {
    let transactions = state.node.inner.get_on_chain_history().await.map_err(|e| {
        AppError::InternalServerError(format!("Failed to list transactions: {e:#}"))
    })?;

    Ok(Json(transactions))
}

pub async fn list_peers(State(state): State<Arc<AppState>>) -> Json<Vec<PublicKey>> {
    let peers = state.node.inner.list_peers();
    Json(peers)
}

#[derive(Debug, Deserialize)]
pub struct CloseChanelParams {
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

#[autometrics]
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
        .send_payment(&invoice)
        .map_err(|e| AppError::InternalServerError(format!("{e:#}")))?;

    Ok(())
}

#[instrument(skip_all, err(Debug))]
#[autometrics]
pub async fn close_channel(
    Path(channel_id_string): Path<String>,
    Query(params): Query<CloseChanelParams>,
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

#[autometrics]
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

#[autometrics]
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

#[autometrics]
pub async fn is_connected(
    State(state): State<Arc<AppState>>,
    Path(target_pubkey): Path<String>,
) -> Result<Json<bool>, AppError> {
    let target = target_pubkey.parse().map_err(|err| {
        AppError::BadRequest(format!("Invalid public key {target_pubkey}. Error: {err}"))
    })?;
    Ok(Json(state.node.is_connected(&target)))
}
