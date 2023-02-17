use crate::trade::ContractSymbolTrade;
use crate::trade::DirectionTrade;
use uuid::Uuid;

pub mod handler;

// When naming this the same as `api_model::order::OrderType` the generated code somehow uses
// `trade::OrderType` and contains errors, hence different name is used.
// This is likely a bug in frb.
#[derive(Debug, Clone, Copy)]
pub enum OrderTypeTrade {
    Market,
    Limit { price: f64 },
}

#[derive(Debug, Clone, Copy)]
pub enum OrderStatusTrade {
    Open,
    Filled,
}

#[derive(Debug, Clone, Copy)]
pub struct OrderTrade {
    pub id: Uuid,
    pub leverage: f64,
    pub quantity: f64,
    pub contract_symbol: ContractSymbolTrade,
    pub direction: DirectionTrade,
    pub order_type: OrderTypeTrade,
    pub status: OrderStatusTrade,
}
