use crate::collaborative_revert;
use crate::db;
use crate::parse_channel_id;
use crate::routes::AppState;
use crate::AppError;
use anyhow::Context;
use axum::extract::Path;
use axum::extract::Query;
use axum::extract::State;
use axum::Json;
use bdk::FeeRate;
use bdk::TransactionDetails;
use bitcoin::secp256k1::PublicKey;
use bitcoin::OutPoint;
use commons::CollaborativeRevertCoordinatorExpertRequest;
use commons::CollaborativeRevertCoordinatorRequest;
use dlc_manager::contract::Contract;
use dlc_manager::subchannel::SubChannel;
use lightning_invoice::Bolt11Invoice;
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
    pub channel_balances: Vec<ChannelBalance>,
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

impl
    From<(
        lightning::ln::channelmanager::ChannelDetails,
        String,
        Vec<lightning::chain::channelmonitor::Balance>,
    )> for ChannelDetails
{
    fn from(
        (channel_details, user_email, balances): (
            lightning::ln::channelmanager::ChannelDetails,
            String,
            Vec<lightning::chain::channelmonitor::Balance>,
        ),
    ) -> Self {
        let balances = balances
            .into_iter()
            .map(|balance| balance.into())
            .collect::<Vec<_>>();

        ChannelDetails {
            channel_details: ln_dlc_node::ChannelDetails::from(channel_details),
            user_email,
            channel_balances: balances,
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
            let balances = if let Some(funding_txo) = channel.funding_txo {
                match state.node.inner.get_channel_balances(funding_txo.txid) {
                    Ok(balances) => balances,
                    Err(error) => {
                        tracing::warn!("Could not load balance for error {error:#}");
                        None
                    }
                }
            } else {
                None
            };

            ChannelDetails::from((channel, user_email, balances.unwrap_or_default()))
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

    let dlc_channels = state.node.inner.list_sub_channels().map_err(|e| {
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

    collaborative_revert::propose_collaborative_revert(
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
