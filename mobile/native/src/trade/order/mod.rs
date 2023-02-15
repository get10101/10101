use crate::api_model;
use crate::common::ContractSymbol;
use crate::common::Direction;
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
    pub contract_symbol: ContractSymbol,
    pub direction: Direction,
    pub order_type: OrderTypeTrade,
    pub status: OrderStatusTrade,
}

impl From<api_model::order::NewOrder> for OrderTrade {
    fn from(value: api_model::order::NewOrder) -> Self {
        OrderTrade {
            id: Uuid::new_v4(),
            leverage: value.leverage,
            quantity: value.quantity,
            contract_symbol: value.contract_symbol,
            direction: value.direction,
            order_type: (*value.order_type).into(),
            status: OrderStatusTrade::Open,
        }
    }
}
