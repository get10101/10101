use crate::collaborative_revert;
use crate::collaborative_revert::propose_collaborative_revert;
use crate::db;
use crate::db::positions::Position;
use crate::parse_channel_id;
use crate::position::models::PositionState;
use crate::routes::AppState;
use crate::AppError;
use anyhow::Context;
use axum::extract::Path;
use axum::extract::Query;
use axum::extract::State;
use axum::Json;
use bdk::FeeRate;
use bdk::TransactionDetails;
use bitcoin::hashes::hex::ToHex;
use bitcoin::secp256k1::PublicKey;
use bitcoin::OutPoint;
use commons::CollaborativeRevertCoordinatorExpertRequest;
use commons::CollaborativeRevertCoordinatorRequest;
use dlc_manager::contract::Contract;
use dlc_manager::subchannel::{SubChannel, SubChannelState};
use lightning_invoice::Bolt11Invoice;
use ln_dlc_node::node::NodeInfo;
use rust_decimal::Decimal;
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
    #[serde(flatten)]
    pub contract_details: Option<ln_dlc_node::ContractDetails>,
    pub user_email: String,
    #[serde(with = "time::serde::rfc3339::option")]
    pub user_registration_timestamp: Option<OffsetDateTime>,
}

impl From<(SubChannel, Option<Contract>, String, Option<OffsetDateTime>)> for DlcChannelDetails {
    fn from(
        (channel_details, contract, user_email, user_registration_timestamp): (
            SubChannel,
            Option<Contract>,
            String,
            Option<OffsetDateTime>,
        ),
    ) -> Self {
        DlcChannelDetails {
            channel_details: ln_dlc_node::DlcChannelDetails::from(channel_details),
            contract_details: contract.map(ln_dlc_node::ContractDetails::from),
            user_email,
            user_registration_timestamp,
        }
    }
}

#[instrument(skip_all, err(Debug))]
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

            let dlc_channel_id = subchannel.get_dlc_channel_id(0);

            let contract = match dlc_channel_id {
                Some(dlc_channel_id) => {
                    match state
                        .node
                        .inner
                        .get_contract_by_dlc_channel_id(&dlc_channel_id)
                    {
                        Ok(contract) => Some(contract),
                        Err(_) => None,
                    }
                }
                None => None,
            };

            DlcChannelDetails::from((subchannel, contract, email, registration_timestamp))
        })
        .collect::<Vec<_>>();

    Ok(Json(dlc_channels))
}

#[derive(Serialize)]
pub struct Details {
    lightning_channels: usize,
    lightning_channels_without_sub_channel_or_with_sub_channel_which_is_off_chain_closed: usize,
    lightning_channels_with_any_sub_channel_which_is_expired_but_not_offchain_closed: usize,
}

#[derive(Deserialize)]
pub struct YoloRevert {
    fee_rate: u64,
    dry_run: bool,
}

#[allow(clippy::unnecessary_filter_map)]
pub async fn revert_everything_yolo(
    State(state): State<Arc<AppState>>,
    query_param: Query<YoloRevert>,
) -> Result<Json<Details>, AppError> {
    let lightning_channels = state.node.inner.list_channels();
    let sub_channels = state
        .node
        .inner
        .list_dlc_channels()
        .expect("Could not fetch sub_channels");

    let lightning_channels = lightning_channels
        .iter()
        .filter(|channel| !channel.is_public)
        .collect::<Vec<_>>();

    tracing::info!(size = lightning_channels.len(), "Lightning channels");

    // 1. collab close all lightning channels without a dlc channel (or with a subchannel which is off-chain closed)

    let open_sub_channels = sub_channels
        .iter()
        .filter(|subchannle| subchannle.state != SubChannelState::OffChainClosed);

    let lightning_channels_without_sub_channel_or_with_sub_channel_which_is_off_chain_closed =
        lightning_channels
            .iter()
            .filter_map(|channel| {
                match sub_channels
                    .iter()
                    .find(|sub_channel| sub_channel.channel_id == channel.channel_id)
                {
                    None => Some(channel),
                    Some(sub_channel) => {
                        if sub_channel.state == SubChannelState::OffChainClosed {
                            Some(channel)
                        } else {
                            None
                        }
                    }
                }
            })
            .collect::<Vec<_>>();

    tracing::info!(
        size = lightning_channels_without_sub_channel_or_with_sub_channel_which_is_off_chain_closed
            .len(),
        "lightning_channels_without_sub_channel_or_with_sub_channel_which_is_off_chain_closed"
    );

    let mut connection = state.node.pool.get().expect("To be abel to get connection");
    let positions = Position::get_all_positions(&mut connection, OffsetDateTime::now_utc())
        .expect("to be able to read from db");
    let open_and_expired = positions
        .iter()
        .filter(|position| !matches!(position.position_state, PositionState::Closed { .. }))
        .collect::<Vec<_>>();

    let open_and_expired_sub_channels = open_sub_channels
        .filter(|sub_channel| {
            open_and_expired
                .iter()
                .any(|position| position.trader == sub_channel.counter_party)
        })
        .collect::<Vec<_>>();

    let lightning_channels_with_sub_channel_which_is_not_offchain_closed = lightning_channels
        .iter()
        .filter(|channel| {
            !lightning_channels_without_sub_channel_or_with_sub_channel_which_is_off_chain_closed
                .contains(channel)
        })
        .collect::<Vec<_>>();

    let lightning_channels_with_any_sub_channel_which_is_expired_but_not_offchain_closed =
        lightning_channels_with_sub_channel_which_is_not_offchain_closed
            .iter()
            .filter_map(|channel| {
                if open_and_expired_sub_channels
                    .iter()
                    .any(|sub_channel| sub_channel.channel_id == channel.channel_id)
                {
                    Some(channel)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

    tracing::info!(
        size =
            lightning_channels_with_any_sub_channel_which_is_expired_but_not_offchain_closed.len(),
        "lightning_channels_with_any_sub_channel_which_is_expired_but_not_offchain_closed"
    );

    let lightning_channels_without_sub_channel_or_with_sub_channel_which_is_off_chain_closed_size =
        lightning_channels_without_sub_channel_or_with_sub_channel_which_is_off_chain_closed.len();
    let lightning_channels_with_any_sub_channel_which_is_expired_but_not_offchain_closed_size =
        lightning_channels_with_any_sub_channel_which_is_expired_but_not_offchain_closed.len();
    let channels = lightning_channels.len();

    if lightning_channels_without_sub_channel_or_with_sub_channel_which_is_off_chain_closed_size
        + lightning_channels_with_any_sub_channel_which_is_expired_but_not_offchain_closed_size
        > channels
    {
        return Err(AppError::InternalServerError(
            "Some logic is flawed losers! :P ".to_string(),
        ));
    }

    for channel in
        lightning_channels_without_sub_channel_or_with_sub_channel_which_is_off_chain_closed
    {
        if query_param.dry_run {
            let channel_id = channel.channel_id.to_hex();
            tracing::info!(
                channel_id = channel_id,
                "Would propose collab revert for channel"
            );
            continue;
        }

        if let Err(err) = propose_collaborative_revert(
            state.node.inner.clone(),
            state.pool.clone(),
            state.auth_users_notifier.clone(),
            channel.channel_id,
            Decimal::ONE,
            query_param.fee_rate,
            OutPoint {
                txid: channel.funding_txo.expect("to have funding txo").txid,
                vout: channel.funding_txo.expect("to have funding txo").index as u32,
            },
        )
        .await
        {
            tracing::error!(channel_id = channel_id, "Could not yolo subchannel");
        }
    }

    Ok(Json(Details {
        lightning_channels: channels,
        lightning_channels_without_sub_channel_or_with_sub_channel_which_is_off_chain_closed: lightning_channels_without_sub_channel_or_with_sub_channel_which_is_off_chain_closed_size,
        lightning_channels_with_any_sub_channel_which_is_expired_but_not_offchain_closed: lightning_channels_with_any_sub_channel_which_is_expired_but_not_offchain_closed_size,
    }))
}

#[instrument(skip_all, err(Debug))]
pub async fn collaborative_revert(
    State(state): State<Arc<AppState>>,
    revert_params: Json<CollaborativeRevertCoordinatorRequest>,
) -> Result<(), AppError> {
    let channel_id_hex = revert_params.channel_id.clone();
    let channel_id = parse_channel_id(channel_id_hex.as_str())
        .map_err(|e| AppError::BadRequest(format!("Invalid channel ID provided: {e:#}")))?;

    let funding_txo = OutPoint {
        txid: revert_params.txid,
        vout: revert_params.vout,
    };

    propose_collaborative_revert(
        state.node.inner.clone(),
        state.pool.clone(),
        state.auth_users_notifier.clone(),
        channel_id,
        revert_params.price,
        revert_params.fee_rate_sats_vb,
        funding_txo,
    )
    .await
    .map_err(|e| {
        AppError::InternalServerError(format!("Could not collaboratively revert channel: {e:#}"))
    })?;

    tracing::info!(channel_id = channel_id_hex, "Proposed collaborative revert");

    Ok(())
}

#[instrument(skip_all, err(Debug))]
pub async fn expert_collaborative_revert(
    State(state): State<Arc<AppState>>,
    revert_params: Json<CollaborativeRevertCoordinatorExpertRequest>,
) -> Result<(), AppError> {
    let channel_id_hex = revert_params.channel_id.clone();
    let channel_id = parse_channel_id(channel_id_hex.as_str())
        .map_err(|e| AppError::BadRequest(format!("Invalid channel ID provided: {e:#}")))?;

    let funding_txo = OutPoint {
        txid: revert_params.txid,
        vout: revert_params.vout,
    };

    collaborative_revert::propose_collaborative_revert_without_channel_details(
        state.node.inner.clone(),
        state.pool.clone(),
        state.auth_users_notifier.clone(),
        channel_id,
        funding_txo,
        revert_params.coordinator_amount,
        revert_params.fee_rate_sats_vb,
        revert_params.price,
    )
    .await
    .map_err(|e| {
        AppError::InternalServerError(format!("Could not collaboratively revert channel: {e:#}"))
    })?;

    tracing::info!(channel_id = channel_id_hex, "Proposed collaborative revert");

    Ok(())
}

#[instrument(skip_all, err(Debug))]
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
    /// Defines the fee rate for the channel opening transaction. If not provided, it will default
    /// to system settings
    sats_vbyte: Option<f32>,
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
    let mut pending_channel_opening = state.node.inner.pending_channel_opening_fee_rates.lock();
    if let Some(fee_rate) = channel_params.sats_vbyte {
        pending_channel_opening.insert(pubkey, FeeRate::from_sat_per_vb(fee_rate));
    }

    let channel_id = state
        .node
        .inner
        .initiate_open_channel(pubkey, channel_amount, initial_send_amount, true)
        .map_err(|e| AppError::InternalServerError(format!("Failed to open channel: {e:#}")))?;

    tracing::debug!(
        "Successfully opened channel with {pubkey}. Funding tx: {}",
        hex::encode(channel_id.0)
    );

    Ok(Json(hex::encode(channel_id.0)))
}

#[instrument(skip_all, err(Debug))]
pub async fn send_payment(
    Path(invoice): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<(), AppError> {
    let invoice = Bolt11Invoice::from_str(invoice.as_str())
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
    let channel_id = parse_channel_id(&channel_id_string)
        .map_err(|_| AppError::BadRequest("Provided channel ID was invalid".to_string()))?;

    tracing::info!(channel_id = %channel_id_string, "Attempting to close channel");

    state
        .node
        .inner
        .close_channel(channel_id, params.force.unwrap_or_default())
        .map_err(|e| AppError::InternalServerError(format!("{e:#}")))?;

    Ok(())
}

#[instrument(skip_all, err(Debug))]
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

#[instrument(skip_all, err(Debug))]
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

#[instrument(skip_all, err(Debug))]
pub async fn is_connected(
    State(state): State<Arc<AppState>>,
    Path(target_pubkey): Path<String>,
) -> Result<Json<bool>, AppError> {
    let target = target_pubkey.parse().map_err(|err| {
        AppError::BadRequest(format!("Invalid public key {target_pubkey}. Error: {err}"))
    })?;
    Ok(Json(state.node.is_connected(&target)))
}
