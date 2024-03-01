use crate::collaborative_revert;
use crate::db;
use crate::parse_dlc_channel_id;
use crate::routes::AppState;
use crate::AppError;
use anyhow::Context;
use axum::extract::Path;
use axum::extract::Query;
use axum::extract::State;
use axum::Json;
use bitcoin::secp256k1::PublicKey;
use bitcoin::Amount;
use bitcoin::OutPoint;
use bitcoin::Transaction;
use bitcoin::TxOut;
use commons::CollaborativeRevertCoordinatorRequest;
use dlc_manager::channel::Channel;
use dlc_manager::contract::Contract;
use dlc_manager::Storage;
use lightning::chain::chaininterface::ConfirmationTarget;
use ln_dlc_node::node::NodeInfo;
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
pub enum ChannelBalance {
    /// The channel is not yet closed (or the commitment or closing transaction has not yet
    /// appeared in a block). The given balance is claimable (less on-chain fees) if the channel is
    /// force-closed now.
    NotYetClosedClaimableOnChannelClose { amount_satoshis: u64 },
    /// The channel has been closed, and the given balance is ours but awaiting confirmations until
    /// we consider it spendable.
    ClaimableAwaitingConfirmations {
        amount_satoshis: u64,
        confirmation_height: u32,
    },
    /// The channel has been closed, and the given balance should be ours but awaiting spending
    /// transaction confirmation. If the spending transaction does not confirm in time, it is
    /// possible our counterparty can take the funds by broadcasting an HTLC timeout on-chain.
    ///
    /// Once the spending transaction confirms, before it has reached enough confirmations to be
    /// considered safe from chain reorganizations, the balance will instead be provided via
    /// [`Balance::ClaimableAwaitingConfirmations`].
    ContentiousClaimable {
        amount_satoshis: u64,
        timeout_height: u32,
        payment_hash: String,
        payment_preimage: String,
    },
    /// HTLCs which we sent to our counterparty which are claimable after a timeout (less on-chain
    /// fees) if the counterparty does not know the preimage for the HTLCs. These are somewhat
    /// likely to be claimed by our counterparty before we do.
    MaybeTimeoutClaimableHTLC {
        amount_satoshis: u64,
        claimable_height: u32,
        payment_hash: String,
    },
    /// HTLCs which we received from our counterparty which are claimable with a preimage which we
    /// do not currently have. This will only be claimable if we receive the preimage from the node
    /// to which we forwarded this HTLC before the timeout.
    MaybePreimageClaimableHTLC {
        amount_satoshis: u64,
        expiry_height: u32,
        payment_hash: String,
    },
    /// The channel has been closed, and our counterparty broadcasted a revoked commitment
    /// transaction.
    ///
    /// Thus, we're able to claim all outputs in the commitment transaction, one of which has the
    /// following amount.
    CounterpartyRevokedOutputClaimable { amount_satoshis: u64 },
}

impl From<lightning::chain::channelmonitor::Balance> for ChannelBalance {
    fn from(value: lightning::chain::channelmonitor::Balance) -> Self {
        match value {
            lightning::chain::channelmonitor::Balance::ClaimableOnChannelClose {
                amount_satoshis,
            } => ChannelBalance::NotYetClosedClaimableOnChannelClose { amount_satoshis },
            lightning::chain::channelmonitor::Balance::ClaimableAwaitingConfirmations {
                amount_satoshis,
                confirmation_height,
            } => ChannelBalance::ClaimableAwaitingConfirmations {
                amount_satoshis,
                confirmation_height,
            },
            lightning::chain::channelmonitor::Balance::ContentiousClaimable {
                amount_satoshis,
                timeout_height,
                payment_hash,
                payment_preimage,
            } => ChannelBalance::ContentiousClaimable {
                payment_hash: payment_hash.to_string(),
                payment_preimage: payment_preimage.to_string(),
                amount_satoshis,
                timeout_height,
            },
            lightning::chain::channelmonitor::Balance::MaybeTimeoutClaimableHTLC {
                amount_satoshis,
                claimable_height,
                payment_hash,
            } => ChannelBalance::MaybeTimeoutClaimableHTLC {
                amount_satoshis,
                claimable_height,
                payment_hash: payment_hash.to_string(),
            },
            lightning::chain::channelmonitor::Balance::MaybePreimageClaimableHTLC {
                amount_satoshis,
                expiry_height,
                payment_hash,
            } => ChannelBalance::MaybePreimageClaimableHTLC {
                amount_satoshis,
                expiry_height,
                payment_hash: payment_hash.to_string(),
            },
            lightning::chain::channelmonitor::Balance::CounterpartyRevokedOutputClaimable {
                amount_satoshis,
            } => ChannelBalance::CounterpartyRevokedOutputClaimable { amount_satoshis },
        }
    }
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

impl From<(Channel, Option<Contract>, String, Option<OffsetDateTime>)> for DlcChannelDetails {
    fn from(
        (channel_details, contract, user_email, user_registration_timestamp): (
            Channel,
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

            DlcChannelDetails::from((dlc_channel, contract, email, registration_timestamp))
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

#[instrument(skip_all, err(Debug))]
pub async fn close_channel(
    Path(channel_id_string): Path<String>,
    Query(params): Query<CloseChannelParams>,
    State(state): State<Arc<AppState>>,
) -> Result<(), AppError> {
    let channel_id = parse_dlc_channel_id(&channel_id_string)
        .map_err(|_| AppError::BadRequest("Provided channel ID was invalid".to_string()))?;

    tracing::info!(channel_id = %channel_id_string, "Attempting to close channel");

    state
        .node
        .inner
        .close_dlc_channel(channel_id, params.force.unwrap_or_default())
        .await
        .map_err(|e| AppError::InternalServerError(format!("{e:#}")))?;

    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct DeleteDlcChannel {
    #[serde(default, deserialize_with = "empty_string_as_none")]
    i_know_what_i_am_doing: Option<bool>,
}

/// This function deletes a DLC channel from our database irreversible!
/// If you want to close a channel instead, use `close_channel`
#[instrument(skip_all, err(Debug))]
pub async fn delete_dlc_channels(
    Path(channel_id_string): Path<String>,
    State(state): State<Arc<AppState>>,
    Query(params): Query<DeleteDlcChannel>,
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

impl From<ln_dlc_node::TransactionDetails> for TransactionDetails {
    fn from(value: ln_dlc_node::TransactionDetails) -> Self {
        Self {
            transaction: value.transaction,
            sent: value.sent,
            received: value.received,
            fee: value.fee.ok(),
            confirmation_status: value.confirmation_status.into(),
        }
    }
}

impl From<ln_dlc_node::ConfirmationStatus> for ConfirmationStatus {
    fn from(value: ln_dlc_node::ConfirmationStatus) -> Self {
        match value {
            ln_dlc_node::ConfirmationStatus::Unknown => Self::Unknown,
            ln_dlc_node::ConfirmationStatus::Mempool { last_seen } => Self::Mempool { last_seen },
            ln_dlc_node::ConfirmationStatus::Confirmed {
                n_confirmations,
                timestamp,
            } => Self::Confirmed {
                n_confirmations,
                timestamp,
            },
        }
    }
}
