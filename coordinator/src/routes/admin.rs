use crate::collaborative_revert;
use crate::db;
use crate::funding_fee;
use crate::parse_dlc_channel_id;
use crate::position::models::Position;
use crate::referrals;
use crate::routes::AppState;
use crate::settings::SettingsFile;
use crate::AppError;
use anyhow::Context;
use axum::extract::Path;
use axum::extract::Query;
use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;
use bitcoin::secp256k1::PublicKey;
use bitcoin::Amount;
use bitcoin::OutPoint;
use bitcoin::Transaction;
use bitcoin::TxOut;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::PooledConnection;
use diesel::PgConnection;
use dlc_manager::channel::signed_channel::SignedChannelState;
use dlc_manager::channel::Channel;
use dlc_manager::DlcChannelId;
use dlc_manager::Storage;
use hex::FromHex;
use lightning::chain::chaininterface::ConfirmationTarget;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::de;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use std::cmp::Ordering;
use std::fmt;
use std::num::NonZeroU32;
use std::str::FromStr;
use std::sync::Arc;
use time::OffsetDateTime;
use tokio::task::spawn_blocking;
use tracing::instrument;
use xxi_node::bitcoin_conversion::to_secp_pk_30;
use xxi_node::bitcoin_conversion::to_txid_30;
use xxi_node::commons;
use xxi_node::commons::CollaborativeRevertCoordinatorRequest;
use xxi_node::node::ProtocolId;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Balance {
    pub onchain: u64,
    pub dlc_channel: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TransactionDetails {
    pub transaction: Transaction,
    #[serde(with = "bitcoin::amount::serde::as_sat")]
    pub sent: Amount,
    #[serde(with = "bitcoin::amount::serde::as_sat")]
    pub received: Amount,
    #[serde(with = "bitcoin::amount::serde::as_sat::opt")]
    pub fee: Option<Amount>,
    pub confirmation_status: ConfirmationStatus,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ConfirmationStatus {
    Unknown,
    Mempool {
        #[serde(with = "time::serde::rfc3339")]
        last_seen: OffsetDateTime,
    },
    Confirmed {
        n_confirmations: NonZeroU32,
        #[serde(with = "time::serde::rfc3339")]
        timestamp: OffsetDateTime,
    },
}

pub async fn get_balance(State(state): State<Arc<AppState>>) -> Result<Json<Balance>, AppError> {
    spawn_blocking(move || {
        let dlc_channel = state
            .node
            .inner
            .get_dlc_channels_usable_balance()
            .map_err(|error| {
                AppError::InternalServerError(format!(
                    "Failed getting dlc channel balance {error:#}"
                ))
            })?;
        let onchain = state.node.inner.get_on_chain_balance();

        Ok(Json(Balance {
            onchain: onchain.confirmed,
            dlc_channel: dlc_channel.to_sat(),
        }))
    })
    .await
    .map_err(|e| AppError::InternalServerError(format!("Failed to get balance: {e:#}")))?
}

pub async fn get_utxos(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<(OutPoint, TxOut)>>, AppError> {
    let utxos = state.node.inner.get_utxos();

    Ok(Json(utxos))
}

#[derive(Serialize)]
pub struct FeeRateEstimation(u32);

pub async fn get_fee_rate_estimation(
    State(state): State<Arc<AppState>>,
    Path(target): Path<String>,
) -> Result<Json<FeeRateEstimation>, AppError> {
    let target = match target.as_str() {
        "normal" => ConfirmationTarget::Normal,
        "background" => ConfirmationTarget::Background,
        "highpriority" => ConfirmationTarget::HighPriority,
        "mempoolminimum" => ConfirmationTarget::MempoolMinimum,
        _ => {
            return Err(AppError::BadRequest(
                "Unknown confirmation target".to_string(),
            ));
        }
    };

    let sats_per_vbyte = state
        .node
        .inner
        .fee_rate_estimator
        .get(target)
        .as_sat_per_vb()
        .floor();

    let sats_per_vbyte = Decimal::from_f32(sats_per_vbyte)
        .context("failed to convert f32 to u32")
        .map_err(|e| AppError::InternalServerError(format!("{e:#}")))?;

    let fee_rate = sats_per_vbyte
        .to_u32()
        .context("failed to convert to u32")
        .map_err(|e| AppError::InternalServerError(format!("{e:#}")))?;
    Ok(Json(FeeRateEstimation(fee_rate)))
}

#[derive(Serialize)]
pub struct DlcChannelDetails {
    #[serde(flatten)]
    pub channel_details: xxi_node::DlcChannelDetails,
    #[serde(flatten)]
    pub contract_details: Option<xxi_node::ContractDetails>,
    pub user_email: String,
    #[serde(with = "time::serde::rfc3339::option")]
    pub user_registration_timestamp: Option<OffsetDateTime>,
    #[serde(with = "bitcoin::amount::serde::as_sat::opt")]
    pub coordinator_reserve_sats: Option<Amount>,
    #[serde(with = "bitcoin::amount::serde::as_sat::opt")]
    pub trader_reserve_sats: Option<Amount>,
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

    let mut dlc_channels = dlc_channels
        .into_iter()
        .map(|dlc_channel| {
            let (email, registration_timestamp) =
                match db::user::by_id(&mut conn, dlc_channel.get_counter_party_id().to_string()) {
                    Ok(Some(user)) => (user.contact, Some(user.timestamp)),
                    _ => ("unknown".to_string(), None),
                };

            let dlc_channel_id = dlc_channel.get_id();

            let contract = match state
                .node
                .inner
                .get_contract_by_dlc_channel_id(&dlc_channel_id)
            {
                Ok(contract) => Some(contract),
                Err(_) => None,
            };

            let coordinator_reserve_sats = state
                .node
                .inner
                .get_dlc_channel_usable_balance(&dlc_channel_id)
                .ok();

            let trader_reserve_sats = state
                .node
                .inner
                .get_dlc_channel_usable_balance_counterparty(&dlc_channel_id)
                .ok();

            DlcChannelDetails {
                channel_details: xxi_node::DlcChannelDetails::from(dlc_channel),
                contract_details: contract.map(xxi_node::ContractDetails::from),
                user_email: email,
                user_registration_timestamp: registration_timestamp,
                coordinator_reserve_sats,
                trader_reserve_sats,
            }
        })
        .collect::<Vec<_>>();

    // Sort channels by state
    dlc_channels.sort_by(|a, b| {
        let ordering = a
            .channel_details
            .channel_state
            .cmp(&b.channel_details.channel_state);
        if ordering != Ordering::Equal {
            return ordering;
        }
        a.channel_details
            .signed_channel_state
            .cmp(&b.channel_details.signed_channel_state)
    });

    Ok(Json(dlc_channels))
}

#[instrument(skip_all, err(Debug))]
pub async fn collaborative_revert(
    State(state): State<Arc<AppState>>,
    revert_params: Json<CollaborativeRevertCoordinatorRequest>,
) -> Result<(), AppError> {
    let channel_id_hex = revert_params.channel_id.clone();
    let channel_id = parse_dlc_channel_id(channel_id_hex.as_str())
        .map_err(|e| AppError::BadRequest(format!("Invalid channel ID provided: {e:#}")))?;

    collaborative_revert::propose_collaborative_revert(
        state.node.inner.clone(),
        state.pool.clone(),
        state.auth_users_notifier.clone(),
        channel_id,
        revert_params.fee_rate_sats_vb,
        revert_params.counter_payout,
        revert_params.price,
    )
    .await
    .map_err(|e| {
        AppError::InternalServerError(format!("Could not collaboratively revert channel: {e:#}"))
    })?;

    tracing::info!(channel_id = channel_id_hex, "Proposed collaborative revert");

    Ok(())
}

pub async fn list_on_chain_transactions(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<TransactionDetails>> {
    let transactions = spawn_blocking(move || state.node.inner.get_on_chain_history())
        .await
        .expect("task to complete");

    Json(
        transactions
            .into_iter()
            .map(TransactionDetails::from)
            .collect(),
    )
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

#[instrument(skip_all, err(Debug))]
pub async fn close_channel(
    Path(channel_id_string): Path<String>,
    Query(params): Query<CloseChannelParams>,
    State(state): State<Arc<AppState>>,
) -> Result<(), AppError> {
    let channel_id = parse_dlc_channel_id(&channel_id_string)
        .map_err(|_| AppError::BadRequest("Provided channel ID was invalid".to_string()))?;

    tracing::info!(channel_id = %channel_id_string, "Attempting to close channel");

    match params.force.unwrap_or_default() {
        true => state.node.force_close_dlc_channel(channel_id).await,
        false => state.node.close_dlc_channel(channel_id).await,
    }
    .map_err(|e| AppError::InternalServerError(format!("{e:#}")))?;

    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct Confirmation {
    #[serde(default, deserialize_with = "empty_string_as_none")]
    i_know_what_i_am_doing: Option<bool>,
}

/// This function deletes a DLC channel from our database irreversible!
/// If you want to close a channel instead, use `close_channel`
#[instrument(skip_all, err(Debug))]
pub async fn delete_dlc_channel(
    Path(channel_id_string): Path<String>,
    State(state): State<Arc<AppState>>,
    Query(params): Query<Confirmation>,
) -> Result<(), AppError> {
    if !params.i_know_what_i_am_doing.unwrap_or_default() {
        let error_message =
            "Looks like you don't know what you are doing! Go and ask your supervisor for help!";
        tracing::warn!(error_message);
        return Err(AppError::BadRequest(error_message.to_string()));
    }

    let channel_id = parse_dlc_channel_id(&channel_id_string)
        .map_err(|_| AppError::BadRequest("Provided channel ID was invalid".to_string()))?;

    tracing::info!(channel_id = %channel_id_string, "Deleting dlc channel");

    state
        .node
        .inner
        .dlc_storage
        .delete_channel(&channel_id)
        .map_err(|e| {
            AppError::InternalServerError(format!(
                "Could not delete dlc_channel with id {} due to {:?}",
                channel_id_string, e
            ))
        })?;

    tracing::info!(channel_id = %channel_id_string, "Deleted dlc channel");

    Ok(())
}

/// This function attempts to roll back a DLC channel to the last stable state!
/// The action is irreversible, only use if you know what you are doing!
#[instrument(skip_all, err(Debug))]
pub async fn roll_back_dlc_channel(
    Path(channel_id_string): Path<String>,
    State(state): State<Arc<AppState>>,
    Query(params): Query<Confirmation>,
) -> Result<(), AppError> {
    if !params.i_know_what_i_am_doing.unwrap_or_default() {
        let error_message =
            "Looks like you don't know what you are doing! Go and ask your supervisor for help!";
        tracing::warn!(error_message);
        return Err(AppError::BadRequest(error_message.to_string()));
    }

    let channel_id = parse_dlc_channel_id(&channel_id_string)
        .map_err(|_| AppError::BadRequest("Provided channel ID was invalid".to_string()))?;

    tracing::info!(channel_id = %channel_id_string, "Attempting to roll back dlc channel to last stable state");

    let channel = state
        .node
        .inner
        .get_dlc_channel_by_id(&channel_id)
        .map_err(|e| AppError::BadRequest(format!("Couldn't find channel. {e:#}")))?;
    if let Channel::Signed(signed_channel) = channel {
        state
            .node
            .inner
            .roll_back_channel(&signed_channel)
            .map_err(|e| {
                AppError::InternalServerError(format!("Failed to roll back channel. {e:#}"))
            })?
    } else {
        return Err(AppError::BadRequest(
            "It's only possible to rollback a channel in state signed".to_string(),
        ));
    }

    tracing::info!(channel_id = %channel_id_string, "Rolled back dlc channel");

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

    Ok(Json(state.node.is_connected(target)))
}

#[instrument(skip_all, err(Debug))]
pub async fn rollover(
    State(state): State<Arc<AppState>>,
    Path(dlc_channel_id): Path<String>,
) -> Result<(), AppError> {
    let dlc_channel_id = DlcChannelId::from_hex(dlc_channel_id.clone()).map_err(|e| {
        AppError::InternalServerError(format!("Could not decode DLC channel ID: {e}"))
    })?;

    let mut connection = state
        .pool
        .get()
        .map_err(|e| AppError::InternalServerError(format!("Could not acquire DB lock: {e}")))?;

    let position = get_position_by_channel_id(&state, dlc_channel_id, &mut connection)
        .map_err(|e| AppError::BadRequest(format!("Could not find position for channel: {e:#}")))?;

    state
        .node
        .propose_rollover(
            &mut connection,
            &dlc_channel_id,
            position,
            state.node.inner.network,
        )
        .await
        .map_err(|e| {
            AppError::InternalServerError(format!("Failed to rollover DLC channel: {e:#}",))
        })?;

    Ok(())
}

fn get_position_by_channel_id(
    state: &Arc<AppState>,
    dlc_channel_id: [u8; 32],
    conn: &mut PooledConnection<ConnectionManager<PgConnection>>,
) -> anyhow::Result<Position> {
    let dlc_channels = state.node.inner.list_dlc_channels()?;

    let public_key = dlc_channels
        .iter()
        .find_map(|channel| {
            if channel.get_id() == dlc_channel_id {
                Some(channel.get_counter_party_id())
            } else {
                None
            }
        })
        .context("DLC Channel not found")?;

    let position = db::positions::Position::get_position_by_trader(
        conn,
        PublicKey::from_slice(&public_key.serialize()).expect("to be valid"),
        vec![],
    )?
    .context("Position for channel not found")?;

    Ok(position)
}

// Migrate existing dlc channels. TODO(holzeis): Delete this function after the migration has been
// run in prod.
pub async fn migrate_dlc_channels(State(state): State<Arc<AppState>>) -> Result<(), AppError> {
    let mut conn = state
        .pool
        .get()
        .map_err(|e| AppError::InternalServerError(format!("{e:#}")))?;
    for channel in state
        .node
        .inner
        .list_signed_dlc_channels()
        .map_err(|e| AppError::InternalServerError(format!("{e:#}")))?
    {
        let coordinator_reserve = state
            .node
            .inner
            .get_dlc_channel_usable_balance(&channel.channel_id)
            .map_err(|e| AppError::InternalServerError(format!("{e:#}")))?;
        let trader_reserve = state
            .node
            .inner
            .get_dlc_channel_usable_balance_counterparty(&channel.channel_id)
            .map_err(|e| AppError::InternalServerError(format!("{e:#}")))?;
        let coordinator_funding = Amount::from_sat(channel.own_params.collateral);
        let trader_funding = Amount::from_sat(channel.counter_params.collateral);

        let protocol_id = match channel.reference_id {
            Some(reference_id) => ProtocolId::try_from(reference_id)
                .map_err(|e| AppError::InternalServerError(format!("{e:#}")))?,
            None => ProtocolId::new(),
        };

        db::dlc_channels::insert_pending_dlc_channel(
            &mut conn,
            &protocol_id,
            &channel.channel_id,
            &to_secp_pk_30(channel.counter_party),
        )
        .map_err(|e| AppError::InternalServerError(format!("{e:#}")))?;

        db::dlc_channels::set_dlc_channel_open(
            &mut conn,
            &protocol_id,
            &channel.channel_id,
            to_txid_30(channel.fund_tx.txid()),
            coordinator_reserve,
            trader_reserve,
            coordinator_funding,
            trader_funding,
        )
        .map_err(|e| AppError::InternalServerError(format!("{e:#}")))?;

        match channel.state {
            SignedChannelState::Closing {
                buffer_transaction, ..
            } => {
                db::dlc_channels::set_channel_force_closing(
                    &mut conn,
                    &channel.channel_id,
                    to_txid_30(buffer_transaction.txid()),
                )
                .map_err(|e| AppError::InternalServerError(format!("{e:#}")))?;
            }
            SignedChannelState::SettledClosing {
                settle_transaction, ..
            } => {
                db::dlc_channels::set_channel_force_closing_settled(
                    &mut conn,
                    &channel.channel_id,
                    to_txid_30(settle_transaction.txid()),
                    None,
                )
                .map_err(|e| AppError::InternalServerError(format!("{e:#}")))?;
            }
            SignedChannelState::CollaborativeCloseOffered { close_tx, .. } => {
                db::dlc_channels::set_channel_collab_closing(
                    &mut conn,
                    &channel.channel_id,
                    to_txid_30(close_tx.txid()),
                )
                .map_err(|e| AppError::InternalServerError(format!("{e:#}")))?;
            }
            _ => {} // ignored
        }
    }

    Ok(())
}

pub async fn resend_renew_revoke_message(
    State(state): State<Arc<AppState>>,
    Path(trader_pubkey): Path<String>,
) -> Result<(), AppError> {
    let trader = trader_pubkey.parse().map_err(|err| {
        AppError::BadRequest(format!("Invalid public key {trader_pubkey}. Error: {err}"))
    })?;

    state
        .node
        .resend_renew_revoke_message_internal(trader)
        .map_err(|e| {
            AppError::InternalServerError(format!(
                "Failed to resend renew revoke message for {}: {e:#}",
                trader_pubkey
            ))
        })?;

    Ok(())
}

/// Internal API for syncing the on-chain wallet and the DLC channels.
#[instrument(skip_all, err(Debug))]
pub async fn post_sync(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SyncParams>,
) -> Result<(), AppError> {
    if params.full.unwrap_or(false) {
        tracing::info!("Full sync");

        let stop_gap = params.gap.unwrap_or(20);

        state.node.inner.full_sync(stop_gap).await.map_err(|e| {
            AppError::InternalServerError(format!("Could not full-sync on-chain wallet: {e:#}"))
        })?;
    } else {
        tracing::info!("Regular sync");

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

#[derive(Debug, Deserialize)]
pub struct SyncParams {
    #[serde(default, deserialize_with = "empty_string_as_none")]
    full: Option<bool>,
    #[serde(default, deserialize_with = "empty_string_as_none")]
    gap: Option<usize>,
}

pub async fn get_settings(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let settings = state.settings.read().await;
    serde_json::to_string(&*settings).expect("to be able to serialise settings")
}

#[instrument(skip_all, err(Debug))]
pub async fn update_settings(
    State(state): State<Arc<AppState>>,
    Json(updated_settings): Json<SettingsFile>,
) -> Result<(), AppError> {
    let mut settings = state.settings.write().await;

    settings.update(updated_settings.clone());

    settings
        .write_to_file()
        .await
        .map_err(|e| AppError::InternalServerError(format!("Could not write settings: {e:#}")))?;

    // Forward relevant settings down to the xxi node.
    state.node.inner.update_settings(settings.xxi.clone()).await;

    Ok(())
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

    let trader_pubkey = trader_pubkey
        .as_str()
        .parse()
        .map_err(|_| AppError::BadRequest("Invalid trader id provided".to_string()))?;

    let referral_status =
        referrals::get_referral_status(trader_pubkey, &mut conn).map_err(|err| {
            AppError::InternalServerError(format!("Could not calculate referral state {err:?}"))
        })?;
    Ok(Json(referral_status))
}

#[instrument(skip_all, err(Debug))]
pub async fn post_funding_rates(
    State(state): State<Arc<AppState>>,
    Json(funding_rates): Json<FundingRates>,
) -> Result<(), AppError> {
    spawn_blocking(move || {
        let mut conn = state.pool.get().map_err(|e| {
            AppError::InternalServerError(format!("Could not get connection: {e:#}"))
        })?;

    let funding_rates = funding_rates
        .0
        .iter()
        .copied()
        .map(funding_fee::FundingRate::from)
        .collect::<Vec<_>>();

        Ok(())
    })
    .await
    .expect("task to complete")?;

    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct FundingRates(Vec<FundingRate>);

#[derive(Debug, Deserialize, Clone, Copy)]
pub struct FundingRate {
    rate: Decimal,
    #[serde(with = "time::serde::rfc3339")]
    start_date: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    end_date: OffsetDateTime,
}

impl From<FundingRate> for funding_fee::FundingRate {
    fn from(value: FundingRate) -> Self {
        funding_fee::FundingRate::new(value.rate, value.start_date, value.end_date)
    }
}

impl From<xxi_node::TransactionDetails> for TransactionDetails {
    fn from(value: xxi_node::TransactionDetails) -> Self {
        Self {
            transaction: value.transaction,
            sent: value.sent,
            received: value.received,
            fee: value.fee.ok(),
            confirmation_status: value.confirmation_status.into(),
        }
    }
}

impl From<xxi_node::ConfirmationStatus> for ConfirmationStatus {
    fn from(value: xxi_node::ConfirmationStatus) -> Self {
        match value {
            xxi_node::ConfirmationStatus::Unknown => Self::Unknown,
            xxi_node::ConfirmationStatus::Mempool { last_seen } => Self::Mempool { last_seen },
            xxi_node::ConfirmationStatus::Confirmed {
                n_confirmations,
                timestamp,
            } => Self::Confirmed {
                n_confirmations,
                timestamp,
            },
        }
    }
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
