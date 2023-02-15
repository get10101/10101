use crate::common::ContractSymbol;
use crate::common::Direction;
use crate::trade;
use flutter_rust_bridge::frb;

#[frb]
#[derive(Debug, Clone, Copy)]
pub enum OrderType {
    Market,
    Limit { price: f64 },
}

#[frb]
#[derive(Debug, Clone, Copy)]
pub enum OrderStatus {
    Open,
    Filled,
}

#[frb]
#[derive(Debug, Clone)]
pub struct NewOrder {
    #[frb(non_final)]
    pub leverage: f64,
    #[frb(non_final)]
    pub quantity: f64,
    #[frb(non_final)]
    pub contract_symbol: ContractSymbol,
    #[frb(non_final)]
    pub direction: Direction,
    // Box needed for complex enum, otherwise generated Rust code complains about Default impl
    // missing
    #[frb(non_final)]
    pub order_type: Box<OrderType>,
}

#[frb]
#[derive(Debug, Clone)]
pub struct Order {
    pub leverage: f64,
    pub quantity: f64,
    pub contract_symbol: ContractSymbol,
    pub direction: Direction,
    pub order_type: Box<OrderType>,
    pub status: OrderStatus,
}

impl From<OrderType> for trade::order::OrderTypeTrade {
    fn from(value: OrderType) -> Self {
        match value {
            OrderType::Market => trade::order::OrderTypeTrade::Market,
            OrderType::Limit { price } => trade::order::OrderTypeTrade::Limit { price },
        }
    }
}

impl From<OrderStatus> for trade::order::OrderStatus {
    fn from(value: OrderStatus) -> Self {
        match value {
            OrderStatus::Open => trade::order::OrderStatus::Open,
            OrderStatus::Filled => trade::order::OrderStatus::Filled,
        }
    }
}
