use crate::db;
use crate::db::positions::Position;
use crate::message::OrderbookMessage;
use crate::position;
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
use bitcoin::Amount;
use coordinator_commons::CollaborativeRevert;
use dlc::util::weight_to_fee;
use dlc_manager::subchannel::LNChannelManager;
use dlc_manager::subchannel::SubChannel;
use lightning_invoice::Invoice;
use ln_dlc_node::node::NodeInfo;
use orderbook_commons::Message;
use rust_decimal::prelude::ToPrimitive;
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
use trade::bitmex_client::Quote;

/// The weight for the collaborative close transaction. It's expected to have 1 input (from the fund
/// transaction) and 2 outputs, one for each party.
/// Note: if either party would have a 0 output, the actual weight will be smaller and we will be
/// overspending tx fee.
const COLLABORATIVE_REVERT_TX_WEIGHT: usize = 672;

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
    let channel_id = hex::decode(channel_id_string.clone())
        .map_err(|err| AppError::BadRequest(err.to_string()))?;
    let channel_id: [u8; 32] = channel_id
        .try_into()
        .map_err(|_| AppError::BadRequest("Provided channel ID was invalid".to_string()))?;

    let mut conn =
        state.pool.clone().get().map_err(|e| {
            AppError::InternalServerError(format!("Failed to acquire db lock: {e:#}"))
        })?;

    let channel_details = state
        .node
        .inner
        .channel_manager
        .get_channel_details(&channel_id)
        .context("Could not get channel")
        .map_err(|error| {
            AppError::InternalServerError(format!("No ln channel found: {error:#}"))
        })?;

    let sub_channels = state.node.inner.list_dlc_channels().map_err(|e| {
        AppError::InternalServerError(format!("Failed to list DLC channels: {e:#}"))
    })?;
    let subchannel = sub_channels
        .iter()
        .find(|c| c.channel_id == channel_id)
        .context("Could not find provided channel")
        .map_err(|e| AppError::BadRequest(format!("Channel not found: {e:#}")))?;

    let position = Position::get_position_by_channel_id(&mut conn, channel_id_string.clone())
        .map_err(|error| {
            tracing::error!(
                channel_id = revert_params.channel_id.clone(),
                "Could not get position for channel {error:#}"
            );
            AppError::InternalServerError(format!("Failed to load position from db: {error:#}"))
        })?;

    let settlement_amount = position
        .calculate_settlement_amount(revert_params.price)
        .map_err(|error| {
            AppError::InternalServerError(format!("Could not calculate pnl {error:#}"))
        })?;

    let pnl = position
        .calculate_coordinator_pnl(Quote {
            bid_size: 0,
            ask_size: 0,
            bid_price: revert_params.price,
            ask_price: revert_params.price,
            symbol: "".to_string(),
            timestamp: OffsetDateTime::now_utc(),
        })
        .map_err(|error| {
            AppError::InternalServerError(format!("Could not calculate pnl {error:#}"))
        })?;

    // There is no easy way to get the total tx fee for all subchannel transactions, hence, we
    // estimate it. This transaction fee is shared among both users fairly
    let dlc_channel_fee = calculate_dlc_channel_tx_fees(
        subchannel.fund_value_satoshis,
        pnl,
        channel_details.inbound_capacity_msat / 1000,
        channel_details.outbound_capacity_msat / 1000,
        position.trader_margin,
        position.coordinator_margin,
    );

    // Coordinator's amount is the total channel's value (fund_value_satoshis) whatever the taker
    // had (inbound_capacity), the taker's PnL (settlement_amount) and the transaction fee
    let coordinator_amount = subchannel.fund_value_satoshis as i64
        - (channel_details.inbound_capacity_msat / 1000) as i64
        - settlement_amount as i64
        - (dlc_channel_fee as f64 / 2.0) as i64;
    let trader_amount = subchannel.fund_value_satoshis - coordinator_amount as u64;

    let fee = weight_to_fee(
        COLLABORATIVE_REVERT_TX_WEIGHT,
        revert_params.fee_rate_sats_vb,
    )
    .expect("To be able to calculate constant fee rate");
    let coordinator_addrss = state.node.inner.get_unused_address();
    let coordinator_amount = Amount::from_sat(coordinator_amount as u64 - fee / 2);
    let trader_amount = Amount::from_sat(trader_amount - fee / 2);

    // TODO: check if trader still has more than dust
    tracing::info!(
        channel_id = channel_id_string,
        coordinator_address = %coordinator_addrss,
        coordinator_amount = coordinator_amount.to_sat(),
        trader_amount = trader_amount.to_sat(),
        "Proposing collaborative revert");

    db::collaborative_reverts::insert(
        &mut conn,
        position::models::CollaborativeRevert {
            channel_id,
            trader_pubkey: position.trader,
            price: revert_params.price.to_f32().expect("to fit into f32"),
            coordinator_address: coordinator_addrss.clone(),
            coordinator_amount_sats: coordinator_amount,
            trader_amount_sats: trader_amount,
            timestamp: OffsetDateTime::now_utc(),
        },
    )
    .map_err(|err| {
        let error_msg = format!("Could not insert new collaborative revert {err:#}");
        tracing::error!("{}", error_msg);

        AppError::InternalServerError(error_msg)
    })?;

    // try to notify user
    state
        .auth_users_notifier
        .send(OrderbookMessage::CollaborativeRevert {
            trader_id: position.trader,
            message: Message::CollaborativeRevert {
                channel_id,
                coordinator_address: coordinator_addrss,
                coordinator_amount,
                trader_amount,
            },
        })
        .await
        .map_err(|error| {
            AppError::InternalServerError(format!("Could not get notify trader {error:#}"))
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
        .send_payment(&invoice)
        .map_err(|e| AppError::InternalServerError(format!("{e:#}")))?;

    Ok(())
}

#[instrument(skip_all, err(Debug))]
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

fn calculate_dlc_channel_tx_fees(
    initial_funding: u64,
    pnl: i64,
    inbound_capacity: u64,
    outbound_capacity: u64,
    trader_margin: i64,
    coordinator_margin: i64,
) -> u64 {
    initial_funding
        - (inbound_capacity
            + outbound_capacity
            + (trader_margin - pnl) as u64
            + (coordinator_margin + pnl) as u64)
}

#[cfg(test)]
pub mod tests {
    use crate::admin::calculate_dlc_channel_tx_fees;

    #[test]
    pub fn calculate_transaction_fee_for_dlc_channel_transactions() {
        let total_fee =
            calculate_dlc_channel_tx_fees(200_000, -4047, 65_450, 85_673, 18_690, 18_690);
        assert_eq!(total_fee, 11_497);
    }
}
