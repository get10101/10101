use crate::routes::AppState;
use crate::AppError;
use axum::extract::Path;
use axum::extract::Query;
use axum::extract::State;
use axum::Json;
use bdk::TransactionDetails;
use bitcoin::secp256k1::PublicKey;
use dlc_manager::Storage;
use ln_dlc_node::node::NodeInfo;
use ln_dlc_node::ChannelDetails;
use ln_dlc_node::DlcChannelDetails;
use serde::de;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

#[derive(Serialize, Deserialize)]
pub struct Balance {
    offchain: u64,
    onchain: u64,
}

pub async fn get_balance(State(state): State<Arc<AppState>>) -> Result<Json<Balance>, AppError> {
    let offchain = state.node.inner.get_ldk_balance();
    let onchain = state
        .node
        .inner
        .get_on_chain_balance()
        .await
        .map_err(|e| AppError::InternalServerError(format!("Failed to get balance: {e:#}")))?;
    Ok(Json(Balance {
        offchain: offchain.available,
        onchain: onchain.confirmed,
    }))
}

pub async fn list_channels(State(state): State<Arc<AppState>>) -> Json<Vec<ChannelDetails>> {
    let channels = state
        .node
        .inner
        .list_channels()
        .into_iter()
        .map(ChannelDetails::from)
        .collect::<Vec<_>>();

    Json(channels)
}

pub async fn list_dlc_channels(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<DlcChannelDetails>>, AppError> {
    let dlc_channels = state.node.inner.list_dlc_channels().map_err(|e| {
        AppError::InternalServerError(format!("Failed to list DLC channels: {e:#}"))
    })?;

    let dlc_channels = dlc_channels
        .into_iter()
        .map(DlcChannelDetails::from)
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

pub async fn close_channel(
    Path(channel_id): Path<String>,
    Query(params): Query<CloseChanelParams>,
    State(state): State<Arc<AppState>>,
) -> Result<(), AppError> {
    let byte_array =
        hex::decode(channel_id.clone()).map_err(|err| AppError::BadRequest(err.to_string()))?;

    if byte_array.len() > 32 {
        return Err(AppError::BadRequest(
            "Provided channel id was invalid".to_string(),
        ));
    }
    // Create a fixed-length byte array of size 8
    let mut fixed_length_array = [0u8; 32];

    // Copy the decoded bytes to the fixed-length array
    let length = std::cmp::min(byte_array.len(), fixed_length_array.len());
    fixed_length_array[..length].copy_from_slice(&byte_array[..length]);

    tracing::debug!("Attempting to close channel {channel_id}");

    state
        .node
        .inner
        .close_channel(fixed_length_array, params.force.unwrap_or_default())
        .map_err(|error| AppError::InternalServerError(error.to_string()))?;

    tracing::info!(%channel_id, "Deleted lightning channel");

    Ok(())
}

pub async fn delete_subchannel(
    Path(channel_id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<(), AppError> {
    let byte_array =
        hex::decode(channel_id.clone()).map_err(|err| AppError::BadRequest(err.to_string()))?;

    if byte_array.len() > 32 {
        return Err(AppError::BadRequest(
            "Provided channel id was invalid".to_string(),
        ));
    }
    // Create a fixed-length byte array of size 8
    let mut fixed_length_array = [0u8; 32];

    // Copy the decoded bytes to the fixed-length array
    let length = std::cmp::min(byte_array.len(), fixed_length_array.len());
    fixed_length_array[..length].copy_from_slice(&byte_array[..length]);

    tracing::debug!(%channel_id, "Attempting to delete DLC channel");

    state
        .node
        .inner
        .sub_channel_manager
        .get_dlc_manager()
        .get_store()
        .delete_subchannel(&fixed_length_array)
        .map_err(|error| {
            AppError::InternalServerError(format!("Unable to delete channel: {error:#}"))
        })?;

    tracing::info!(%channel_id, "Deleted DLC channel");

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
