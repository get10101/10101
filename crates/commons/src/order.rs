use rust_decimal::Decimal;
use secp256k1::PublicKey;
use serde::Deserialize;
use serde::Serialize;
use time::OffsetDateTime;
use trade::ContractSymbol;
use trade::Direction;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone)]
pub struct NewOrder {
    pub id: Uuid,
    pub contract_symbol: ContractSymbol,
    #[serde(with = "rust_decimal::serde::float")]
    pub price: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub quantity: Decimal,
    pub trader_id: PublicKey,
    pub direction: Direction,
    pub leverage: f32,
    pub order_type: OrderType,
    pub expiry: OffsetDateTime,
    pub stable: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum OrderType {
    #[allow(dead_code)]
    Market,
    Limit,
}

#[derive(Deserialize)]
pub struct OrderResponse {
    pub id: Uuid,
    #[serde(with = "rust_decimal::serde::float")]
    pub price: Decimal,
    pub trader_id: PublicKey,
    pub direction: Direction,
    #[serde(with = "rust_decimal::serde::float")]
    pub quantity: Decimal,
    pub order_type: OrderType,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum OrderState {
    Open,
    Matched,
    Taken,
    Failed,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum OrderReason {
    Manual,
    Expired,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Order {
    pub id: Uuid,
    #[serde(with = "rust_decimal::serde::float")]
    pub price: Decimal,
    pub leverage: f32,
    pub contract_symbol: ContractSymbol,
    pub trader_id: PublicKey,
    pub direction: Direction,
    #[serde(with = "rust_decimal::serde::float")]
    pub quantity: Decimal,
    pub order_type: OrderType,
    #[serde(with = "time::serde::rfc3339")]
    pub timestamp: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub expiry: OffsetDateTime,
    pub order_state: OrderState,
    pub order_reason: OrderReason,
    pub stable: bool,
}
