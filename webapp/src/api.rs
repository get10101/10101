use crate::AppState;
use anyhow::anyhow;
use anyhow::Context;
use anyhow::Result;
use axum::extract::Path;
use axum::extract::Query;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::routing::get;
use axum::routing::post;
use axum::Json;
use axum::Router;
use bitcoin::Amount;
use commons::order_matching_fee;
use commons::ChannelOpeningParams;
use commons::Price;
use native::api::ContractSymbol;
use native::api::Direction;
use native::api::Fee;
use native::api::WalletHistoryItemType;
use native::calculations::calculate_pnl;
use native::channel_trade_constraints;
use native::ln_dlc;
use native::ln_dlc::is_dlc_channel_confirmed;
use native::trade::order::FailureReason;
use native::trade::order::InvalidSubchannelOffer;
use native::trade::order::OrderType;
use native::trade::position::PositionState;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::de;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;
use time::OffsetDateTime;
use uuid::Uuid;

pub fn router(app_state: AppState) -> Router {
    Router::new()
        .route("/api/balance", get(get_balance))
        .route("/api/newaddress", get(get_unused_address))
        .route("/api/sendpayment", post(send_payment))
        .route("/api/history", get(get_onchain_payment_history))
        .route("/api/orders", get(get_orders).post(post_new_order))
        .route("/api/positions", get(get_positions))
        .route("/api/quotes/:contract_symbol", get(get_best_quote))
        .route("/api/node", get(get_node_id))
        .route("/api/sync", post(post_sync))
        .route("/api/seed", get(get_seed_phrase))
        .route("/api/channels", get(get_channels).delete(close_channel))
        .route("/api/tradeconstraints", get(get_trade_constraints))
        .with_state(Arc::new(app_state))
}

pub struct AppError(anyhow::Error);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Something went wrong: {}", self.0),
        )
            .into_response()
    }
}

impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

#[derive(Serialize)]
pub struct Version {
    version: String,
    commit_hash: String,
    branch: String,
}

pub async fn version() -> Json<Version> {
    Json(Version {
        version: env!("CARGO_PKG_VERSION").to_string(),
        commit_hash: env!("COMMIT_HASH").to_string(),
        branch: env!("BRANCH_NAME").to_string(),
    })
}

pub async fn get_unused_address() -> Result<impl IntoResponse, AppError> {
    let address = ln_dlc::get_unused_address()?;

    Ok(address)
}

#[derive(Serialize)]
pub struct Balance {
    on_chain: u64,
    off_chain: Option<u64>,
}

pub async fn get_balance(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Option<Balance>>, AppError> {
    let subscribers = &state.subscribers;
    let balance = subscribers.wallet_info().map(|wallet_info| Balance {
        on_chain: wallet_info.balances.on_chain,
        off_chain: wallet_info.balances.off_chain,
    });

    Ok(Json(balance))
}

#[derive(Serialize)]
pub struct OnChainPayment {
    flow: String,
    amount: u64,
    timestamp: u64,
    txid: String,
    confirmations: u64,
    fee: Option<u64>,
}

pub async fn get_onchain_payment_history(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<OnChainPayment>>, AppError> {
    let subscribers = &state.subscribers;
    let history = match subscribers.wallet_info() {
        Some(wallet_info) => wallet_info
            .history
            .into_iter()
            .filter_map(|item| match item.wallet_type {
                WalletHistoryItemType::OnChain {
                    txid,
                    fee_sats,
                    confirmations,
                } => Some(OnChainPayment {
                    flow: item.flow.to_string(),
                    amount: item.amount_sats,
                    timestamp: item.timestamp,
                    txid,
                    confirmations,
                    fee: fee_sats,
                }),
                _ => None,
            })
            .collect::<Vec<OnChainPayment>>(),
        None => vec![],
    };

    Ok(Json(history))
}

#[derive(Deserialize)]
pub struct Payment {
    address: String,
    amount: u64,
    fee: u64,
}

pub async fn send_payment(
    State(state): State<Arc<AppState>>,
    Json(params): Json<Payment>,
) -> Result<(), AppError> {
    if !state.withdrawal_addresses.contains(&params.address)
        && !ln_dlc::is_address_mine(&params.address)?
        && state.whitelist_withdrawal_addresses
    {
        // if whitelisting is configured, the address is not whitelisted and not our own address we
        // prevent the withdrawal.
        return Err(anyhow!("Withdrawal address is not whitelisted!").into());
    }

    ln_dlc::send_payment(
        params.amount,
        params.address,
        Fee::FeeRate { sats: params.fee },
    )
    .await?;

    ln_dlc::refresh_wallet_info().await?;
    Ok(())
}

pub async fn get_node_id() -> impl IntoResponse {
    ln_dlc::get_node_pubkey().to_string()
}

#[derive(Serialize)]
pub struct Seed {
    seed: Vec<String>,
}

pub async fn get_seed_phrase() -> Json<Seed> {
    Json(Seed {
        seed: ln_dlc::get_seed_phrase(),
    })
}

#[derive(Serialize)]
pub struct OrderId {
    id: Uuid,
}

#[derive(Deserialize, Clone)]
pub struct NewOrderParams {
    #[serde(with = "rust_decimal::serde::float")]
    pub leverage: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub quantity: Decimal,
    pub direction: Direction,
    #[serde(with = "bitcoin::amount::serde::as_sat::opt")]
    pub coordinator_reserve: Option<Amount>,
    #[serde(with = "bitcoin::amount::serde::as_sat::opt")]
    pub trader_reserve: Option<Amount>,
}

impl TryFrom<NewOrderParams> for native::trade::order::Order {
    type Error = anyhow::Error;
    fn try_from(value: NewOrderParams) -> Result<Self> {
        Ok(native::trade::order::Order {
            id: Uuid::new_v4(),
            leverage: value
                .leverage
                .to_f32()
                .context("To be able to parse leverage into f32")?,
            quantity: value
                .quantity
                .to_f32()
                .context("To be able to parse leverage into f32")?,
            contract_symbol: ContractSymbol::BtcUsd,
            direction: value.direction,
            // We only support market orders for now
            order_type: OrderType::Market,
            state: native::trade::order::OrderState::Initial,
            creation_timestamp: OffsetDateTime::now_utc(),
            // We do not support setting order expiry from the frontend for now
            order_expiry_timestamp: OffsetDateTime::now_utc() + time::Duration::minutes(1),
            reason: native::trade::order::OrderReason::Manual,
            stable: false,
            failure_reason: None,
        })
    }
}

pub async fn post_new_order(params: Json<NewOrderParams>) -> Result<Json<OrderId>, AppError> {
    let order: native::trade::order::Order = params
        .clone()
        .0
        .try_into()
        .context("Could not parse order request")?;

    let is_dlc_channel_confirmed = is_dlc_channel_confirmed()?;

    let channel_opening_params = if is_dlc_channel_confirmed {
        None
    } else {
        Some(ChannelOpeningParams {
            coordinator_reserve: params.coordinator_reserve.unwrap_or_default(),
            trader_reserve: params.trader_reserve.unwrap_or_default(),
        })
    };

    let order_id =
        native::trade::order::handler::submit_order(order, channel_opening_params).await?;

    Ok(Json(OrderId { id: order_id }))
}

#[derive(Debug, Clone, Serialize)]
pub struct Position {
    pub leverage: f32,
    pub quantity: f32,
    pub contract_symbol: ContractSymbol,
    pub direction: Direction,
    pub average_entry_price: f32,
    pub liquidation_price: f32,
    pub position_state: PositionState,
    pub collateral: u64,
    #[serde(with = "time::serde::rfc3339")]
    pub expiry: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub created: OffsetDateTime,
    pub stable: bool,
    pub pnl_sats: Option<i64>,
    #[serde(with = "bitcoin::amount::serde::as_sat::opt")]
    pub closing_fee: Option<Amount>,
}

impl From<(native::trade::position::Position, Option<Price>)> for Position {
    fn from((position, price): (native::trade::position::Position, Option<Price>)) -> Self {
        let res = price.map(|price| match (price.ask, price.bid) {
            (Some(ask), Some(bid)) => {
                let price = match position.direction {
                    Direction::Long => price.bid,
                    Direction::Short => price.ask,
                };

                // FIXME: A from implementation should not contain this kind of logic.
                let fee_rate = ln_dlc::get_order_matching_fee_rate();

                (
                    calculate_pnl(
                        position.average_entry_price,
                        trade::Price { bid, ask },
                        position.quantity,
                        position.leverage,
                        position.direction,
                    )
                    .ok(),
                    price
                        .map(|price| Some(order_matching_fee(position.quantity, price, fee_rate)))
                        .and_then(|price| price),
                )
            }
            _ => (None, None),
        });

        let (pnl_sats, closing_fee) = match res {
            None => (None, None),
            Some((pnl_sats, closing_fee)) => (pnl_sats, closing_fee),
        };

        Position {
            leverage: position.leverage,
            quantity: position.quantity,
            contract_symbol: position.contract_symbol,
            direction: position.direction,
            average_entry_price: position.average_entry_price,
            liquidation_price: position.liquidation_price,
            position_state: position.position_state,
            collateral: position.collateral,
            expiry: position.expiry,
            updated: position.updated,
            created: position.created,
            stable: position.stable,
            pnl_sats,
            closing_fee,
        }
    }
}

pub async fn get_positions(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<Position>>, AppError> {
    let subscribers = &state.subscribers;
    let ask_price = subscribers.ask_price();
    let bid_price = subscribers.ask_price();

    let positions = native::trade::position::handler::get_positions()?
        .into_iter()
        .map(|position| {
            let quotes = if let (Some(ask), Some(bid)) = (ask_price, bid_price) {
                Some(Price {
                    bid: Some(bid),
                    ask: Some(ask),
                })
            } else {
                None
            };
            // TODO: we should clean this annoying into up sometimes
            (position, quotes).into()
        })
        .collect::<Vec<Position>>();

    Ok(Json(positions))
}

#[derive(Serialize, Debug)]
pub struct Order {
    pub id: Uuid,
    pub leverage: f32,
    pub quantity: f32,
    /// An order only has a price if it either was filled or if it was a limit order (which is not
    /// implemented yet).
    pub price: Option<f32>,
    pub contract_symbol: ContractSymbol,
    pub direction: Direction,
    pub order_type: OrderType,
    pub state: OrderState,
    #[serde(with = "time::serde::rfc3339")]
    pub creation_timestamp: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub order_expiry_timestamp: OffsetDateTime,
    pub failure_reason: Option<String>,
}

#[derive(Serialize, Debug, Clone)]
pub enum OrderState {
    /// Not submitted to orderbook yet
    Initial,

    /// Rejected by the orderbook upon submission
    Rejected,

    /// Successfully submit to orderbook
    Open,

    /// The orderbook has matched the order and it is being filled
    Filling,

    /// The order failed to be filled
    Failed,

    /// Successfully set up trade
    Filled,
}

impl From<native::trade::order::OrderState> for OrderState {
    fn from(value: native::trade::order::OrderState) -> Self {
        match value {
            native::trade::order::OrderState::Initial => OrderState::Initial,
            native::trade::order::OrderState::Rejected => OrderState::Rejected,
            native::trade::order::OrderState::Open => OrderState::Open,
            native::trade::order::OrderState::Filling { .. } => OrderState::Filling,
            native::trade::order::OrderState::Failed { .. } => OrderState::Failed,
            native::trade::order::OrderState::Filled { .. } => OrderState::Filled,
        }
    }
}
impl From<&native::trade::order::Order> for Order {
    fn from(value: &native::trade::order::Order) -> Self {
        let failure_reason = match &value.failure_reason {
            None => None,
            Some(reason) => {
                let reason = match reason {
                    FailureReason::FailedToSetToFilling => "FailedToSetToFilling",
                    FailureReason::TradeRequest => "TradeRequestFailed",
                    FailureReason::TradeResponse(error) => error.as_str(),
                    FailureReason::CollabRevert => "CollabRevert",
                    FailureReason::OrderNotAcceptable => "OrderNotAcceptable",
                    FailureReason::TimedOut => "TimedOut",
                    FailureReason::InvalidDlcOffer(error) => match error {
                        InvalidSubchannelOffer::Outdated => "OfferOutdated",
                        InvalidSubchannelOffer::UndeterminedMaturityDate => {
                            "OfferUndeterminedMaturityDate"
                        }
                        InvalidSubchannelOffer::Unacceptable => "OfferUnacceptable",
                    },
                    FailureReason::OrderRejected(_) => "OrderRejected",
                    FailureReason::Unknown => "Unknown",
                }
                .to_string();
                Some(reason)
            }
        };

        let mut price = None;

        if let OrderType::Limit { price: limit_price } = value.order_type {
            price.replace(limit_price);
        }

        // Note: we might overwrite a limit price here but this is not an issue because if a limit
        // order has been filled the limit price will be filled price and vice versa
        if let native::trade::order::OrderState::Filled {
            execution_price, ..
        } = value.state
        {
            price.replace(execution_price);
        }

        Order {
            id: value.id,
            leverage: value.leverage,
            quantity: value.quantity,
            price,
            contract_symbol: value.contract_symbol,
            direction: value.direction,
            order_type: value.order_type,
            state: value.state.clone().into(),
            creation_timestamp: value.creation_timestamp,
            order_expiry_timestamp: value.order_expiry_timestamp,
            failure_reason,
        }
    }
}

pub async fn post_sync() -> Result<(), AppError> {
    ln_dlc::refresh_wallet_info().await?;

    Ok(())
}

pub async fn get_orders() -> Result<Json<Vec<Order>>, AppError> {
    let orders = native::trade::order::handler::get_orders_for_ui()
        .await?
        .iter()
        .map(|order| order.into())
        .collect();

    Ok(Json(orders))
}

#[derive(Serialize)]
pub struct BestQuote {
    #[serde(flatten)]
    price: Price,
    #[serde(with = "rust_decimal::serde::float")]
    fee: Decimal,
}

pub async fn get_best_quote(
    State(state): State<Arc<AppState>>,
    // todo: once we support multiple pairs we should use this
    Path(_contract_symbol): Path<ContractSymbol>,
) -> Result<Json<Option<BestQuote>>, AppError> {
    let subscribers = &state.subscribers;
    let ask_price = subscribers.ask_price();
    let bid_price = subscribers.bid_price();

    let quotes = BestQuote {
        price: Price {
            bid: bid_price,
            ask: ask_price,
        },
        fee: ln_dlc::get_order_matching_fee_rate(),
    };

    Ok(Json(Some(quotes)))
}

#[derive(Serialize, Default)]
pub struct DlcChannel {
    pub dlc_channel_id: Option<String>,
    pub contract_id: Option<String>,
    pub channel_state: Option<ChannelState>,
    pub buffer_txid: Option<String>,
    pub settle_txid: Option<String>,
    pub claim_txid: Option<String>,
    pub close_txid: Option<String>,
    pub punish_txid: Option<String>,
    pub fund_txid: Option<String>,
    pub fund_txout: Option<usize>,
    pub fee_rate: Option<u64>,
    pub signed_channel_state: Option<SignedChannelState>,
}

#[derive(Serialize)]
pub enum ChannelState {
    Offered,
    Accepted,
    Signed,
    Closing,
    SettledClosing,
    Closed,
    CounterClosed,
    ClosedPunished,
    CollaborativelyClosed,
    FailedAccept,
    FailedSign,
    Cancelled,
}

#[derive(Serialize)]
pub enum SignedChannelState {
    Established,
    SettledOffered,
    SettledReceived,
    SettledAccepted,
    SettledConfirmed,
    Settled,
    RenewOffered,
    RenewAccepted,
    RenewConfirmed,
    RenewFinalized,
    Closing,
    CollaborativeCloseOffered,
}

pub async fn get_channels() -> Result<Json<Vec<DlcChannel>>, AppError> {
    let channels = ln_dlc::list_dlc_channels()?
        .iter()
        .map(DlcChannel::from)
        .collect();
    Ok(Json(channels))
}

impl From<&dlc_manager::channel::Channel> for DlcChannel {
    fn from(value: &dlc_manager::channel::Channel) -> Self {
        match value {
            dlc_manager::channel::Channel::Offered(o) => DlcChannel {
                contract_id: Some(hex::encode(o.offered_contract_id)),
                channel_state: Some(ChannelState::Offered),
                ..DlcChannel::default()
            },
            dlc_manager::channel::Channel::Accepted(a) => DlcChannel {
                dlc_channel_id: Some(hex::encode(a.channel_id)),
                contract_id: Some(hex::encode(a.accepted_contract_id)),
                channel_state: Some(ChannelState::Accepted),
                ..DlcChannel::default()
            },
            dlc_manager::channel::Channel::Signed(s) => {
                let (signed_channel_state, settle_tx, buffer_tx, close_tx) = match &s.state {
                    dlc_manager::channel::signed_channel::SignedChannelState::Established {
                        buffer_transaction,
                        ..
                    } => (
                        SignedChannelState::Established,
                        None,
                        Some(buffer_transaction),
                        None,
                    ),
                    dlc_manager::channel::signed_channel::SignedChannelState::SettledOffered {
                        ..
                    } => (SignedChannelState::SettledOffered, None, None, None),
                    dlc_manager::channel::signed_channel::SignedChannelState::SettledReceived {
                        ..
                    } => (SignedChannelState::SettledReceived, None, None, None),
                    dlc_manager::channel::signed_channel::SignedChannelState::SettledAccepted {
                        settle_tx,
                        ..
                    } => (
                        SignedChannelState::SettledAccepted,
                        Some(settle_tx),
                        None,
                        None,
                    ),
                    dlc_manager::channel::signed_channel::SignedChannelState::SettledConfirmed { settle_tx, .. } => (
                        SignedChannelState::SettledConfirmed,
                        Some(settle_tx),
                        None,
                        None,
                    ),
                    dlc_manager::channel::signed_channel::SignedChannelState::Settled { settle_tx, .. } => {
                        (SignedChannelState::Settled, Some(settle_tx), None, None)
                    }
                    dlc_manager::channel::signed_channel::SignedChannelState::RenewOffered { .. } => {
                        (SignedChannelState::RenewOffered, None, None, None)
                    }
                    dlc_manager::channel::signed_channel::SignedChannelState::RenewAccepted {
                        buffer_transaction, ..
                    } => (
                        SignedChannelState::RenewAccepted,
                        None,
                        Some(buffer_transaction),
                        None,
                    ),
                    dlc_manager::channel::signed_channel::SignedChannelState::RenewConfirmed {
                        buffer_transaction, ..
                    } => (
                        SignedChannelState::RenewConfirmed,
                        None,
                        Some(buffer_transaction),
                        None,
                    ),
                    dlc_manager::channel::signed_channel::SignedChannelState::RenewFinalized {
                        buffer_transaction, ..
                    } => (
                        SignedChannelState::RenewFinalized,
                        None,
                        Some(buffer_transaction),
                        None,
                    ),
                    dlc_manager::channel::signed_channel::SignedChannelState::Closing {
                        buffer_transaction, ..
                    } => (
                        SignedChannelState::Closing,
                        None,
                        Some(buffer_transaction),
                        None,
                    ),
                    dlc_manager::channel::signed_channel::SignedChannelState::SettledClosing {
                        settle_transaction, ..
                    } => (
                        SignedChannelState::Closing,
                        Some(settle_transaction),
                        None,
                        None,
                    ),
                    dlc_manager::channel::signed_channel::SignedChannelState::CollaborativeCloseOffered { close_tx, .. } => (
                        SignedChannelState::CollaborativeCloseOffered,
                        None,
                        None,
                        Some(close_tx),
                    ),
                };
                DlcChannel {
                    dlc_channel_id: Some(hex::encode(s.channel_id)),
                    contract_id: s.get_contract_id().map(hex::encode),
                    channel_state: Some(ChannelState::Signed),
                    fund_txid: Some(s.fund_tx.txid().to_string()),
                    fund_txout: Some(s.fund_output_index),
                    fee_rate: Some(s.fee_rate_per_vb),
                    buffer_txid: buffer_tx.map(|tx| tx.txid().to_string()),
                    settle_txid: settle_tx.map(|tx| tx.txid().to_string()),
                    close_txid: close_tx.map(|tx| tx.txid().to_string()),
                    signed_channel_state: Some(signed_channel_state),
                    ..DlcChannel::default()
                }
            }
            dlc_manager::channel::Channel::Closing(c) => DlcChannel {
                dlc_channel_id: Some(hex::encode(c.channel_id)),
                contract_id: Some(hex::encode(c.contract_id)),
                channel_state: Some(ChannelState::Closing),
                buffer_txid: Some(c.buffer_transaction.txid().to_string()),
                ..DlcChannel::default()
            },
            dlc_manager::channel::Channel::SettledClosing(c) => DlcChannel {
                dlc_channel_id: Some(hex::encode(c.channel_id)),
                channel_state: Some(ChannelState::SettledClosing),
                settle_txid: Some(c.settle_transaction.txid().to_string()),
                claim_txid: Some(c.claim_transaction.txid().to_string()),
                ..DlcChannel::default()
            },
            dlc_manager::channel::Channel::Closed(c) => DlcChannel {
                dlc_channel_id: Some(hex::encode(c.channel_id)),
                channel_state: Some(ChannelState::Closed),
                close_txid: Some(c.closing_txid.to_string()),
                ..DlcChannel::default()
            },
            dlc_manager::channel::Channel::CounterClosed(c) => DlcChannel {
                dlc_channel_id: Some(hex::encode(c.channel_id)),
                channel_state: Some(ChannelState::CounterClosed),
                close_txid: Some(c.closing_txid.to_string()),
                ..DlcChannel::default()
            },
            dlc_manager::channel::Channel::ClosedPunished(c) => DlcChannel {
                dlc_channel_id: Some(hex::encode(c.channel_id)),
                channel_state: Some(ChannelState::ClosedPunished),
                punish_txid: Some(c.punish_txid.to_string()),
                ..DlcChannel::default()
            },
            dlc_manager::channel::Channel::CollaborativelyClosed(c) => DlcChannel {
                dlc_channel_id: Some(hex::encode(c.channel_id)),
                channel_state: Some(ChannelState::CollaborativelyClosed),
                close_txid: Some(c.closing_txid.to_string()),
                ..DlcChannel::default()
            },
            dlc_manager::channel::Channel::FailedAccept(_) => DlcChannel {
                channel_state: Some(ChannelState::FailedAccept),
                ..DlcChannel::default()
            },
            dlc_manager::channel::Channel::FailedSign(c) => DlcChannel {
                dlc_channel_id: Some(hex::encode(c.channel_id)),
                channel_state: Some(ChannelState::FailedSign),
                ..DlcChannel::default()
            },
            dlc_manager::channel::Channel::Cancelled(o) => DlcChannel {
                contract_id: Some(hex::encode(o.offered_contract_id)),
                channel_state: Some(ChannelState::Cancelled),
                ..DlcChannel::default()
            },
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct DeleteChannel {
    #[serde(default, deserialize_with = "empty_string_as_none")]
    force: Option<bool>,
}

pub async fn close_channel(Query(params): Query<DeleteChannel>) -> Result<(), AppError> {
    ln_dlc::close_channel(params.force.unwrap_or_default()).await?;
    Ok(())
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

#[derive(Serialize, Copy, Clone, Debug)]
pub struct TradeConstraints {
    pub max_local_margin_sats: u64,
    pub max_counterparty_margin_sats: u64,
    pub coordinator_leverage: f32,
    pub min_quantity: u64,
    pub is_channel_balance: bool,
    pub min_margin_sats: u64,
    pub estimated_funding_tx_fee_sats: u64,
    pub channel_fee_reserve_sats: u64,
}

pub async fn get_trade_constraints() -> Result<Json<TradeConstraints>, AppError> {
    let trade_constraints = channel_trade_constraints::channel_trade_constraints()?;
    let fee = ln_dlc::estimated_funding_tx_fee()?;
    let channel_fee_reserve = ln_dlc::estimated_fee_reserve()?;
    Ok(Json(TradeConstraints {
        max_local_margin_sats: trade_constraints.max_local_margin_sats,
        max_counterparty_margin_sats: trade_constraints.max_counterparty_margin_sats,
        coordinator_leverage: trade_constraints.coordinator_leverage,
        min_quantity: trade_constraints.min_quantity,
        is_channel_balance: trade_constraints.is_channel_balance,
        min_margin_sats: trade_constraints.min_margin,
        estimated_funding_tx_fee_sats: fee.to_sat(),
        channel_fee_reserve_sats: channel_fee_reserve.to_sat(),
    }))
}
