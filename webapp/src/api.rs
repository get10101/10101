use crate::subscribers::AppSubscribers;
use anyhow::Context;
use anyhow::Result;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::Json;
use native::api::ContractSymbol;
use native::api::Direction;
use native::api::Fee;
use native::api::SendPayment;
use native::api::WalletHistoryItemType;
use native::ln_dlc;
use native::trade::order::OrderState;
use native::trade::order::OrderType;
use native::trade::position;
use native::trade::position::Position;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::Deserialize;
use serde::Serialize;
use std::sync::Arc;
use time::OffsetDateTime;
use uuid::Uuid;

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
}

pub async fn version() -> Json<Version> {
    Json(Version {
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

pub async fn get_unused_address() -> impl IntoResponse {
    ln_dlc::get_unused_address()
}

#[derive(Serialize)]
pub struct Balance {
    on_chain: u64,
    off_chain: u64,
}

pub async fn get_balance(
    State(subscribers): State<Arc<AppSubscribers>>,
) -> Result<Json<Balance>, AppError> {
    ln_dlc::refresh_wallet_info().await?;
    let balance = subscribers
        .wallet_info()
        .map(|wallet_info| Balance {
            on_chain: wallet_info.balances.on_chain,
            off_chain: wallet_info.balances.off_chain,
        })
        .unwrap_or(Balance {
            on_chain: 0,
            off_chain: 0,
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
    State(subscribers): State<Arc<AppSubscribers>>,
) -> Result<Json<Vec<OnChainPayment>>, AppError> {
    ln_dlc::refresh_wallet_info().await?;

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

pub async fn send_payment(params: Json<Payment>) -> Result<(), AppError> {
    ln_dlc::send_payment(SendPayment::OnChain {
        address: params.0.address,
        amount: params.0.amount,
        fee: Fee::FeeRate { sats: params.0.fee },
    })
    .await?;

    Ok(())
}

#[derive(Serialize)]
pub struct OrderId {
    id: Uuid,
}

#[derive(Deserialize)]
pub struct NewOrderParams {
    #[serde(with = "rust_decimal::serde::float")]
    pub leverage: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub quantity: Decimal,
    pub direction: Direction,
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
            state: OrderState::Initial,
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
    let order_id = native::trade::order::handler::submit_order(
        params
            .0
            .try_into()
            .context("Could not parse order request")?,
    )
    .await?;

    Ok(Json(OrderId { id: order_id }))
}

pub async fn get_positions() -> Result<Json<Vec<Position>>, AppError> {
    let positions = position::handler::get_positions()?
        .into_iter()
        .collect::<Vec<Position>>();

    Ok(Json(positions))
}
