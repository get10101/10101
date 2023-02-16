use crate::common::ContractSymbol;
use crate::common::Direction;
use crate::trade;
use flutter_rust_bridge::frb;

pub mod notifications;

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

impl From<OrderStatus> for trade::order::OrderStatusTrade {
    fn from(value: OrderStatus) -> Self {
        match value {
            OrderStatus::Open => trade::order::OrderStatusTrade::Open,
            OrderStatus::Filled => trade::order::OrderStatusTrade::Filled,
        }
    }
}

impl From<trade::order::OrderTrade> for Order {
    fn from(value: trade::order::OrderTrade) -> Self {
        Order {
            leverage: value.leverage,
            quantity: value.quantity,
            contract_symbol: value.contract_symbol,
            direction: value.direction,
            order_type: Box::new(value.order_type.into()),
            status: value.status.into(),
        }
    }
}

impl From<trade::order::OrderTypeTrade> for OrderType {
    fn from(value: trade::order::OrderTypeTrade) -> Self {
        match value {
            trade::order::OrderTypeTrade::Market => OrderType::Market,
            trade::order::OrderTypeTrade::Limit { price } => OrderType::Limit { price },
        }
    }
}

impl From<trade::order::OrderStatusTrade> for OrderStatus {
    fn from(value: trade::order::OrderStatusTrade) -> Self {
        match value {
            trade::order::OrderStatusTrade::Open => OrderStatus::Open,
            trade::order::OrderStatusTrade::Filled => OrderStatus::Filled,
            // When fetching orders from the database we ignore initial and failed orders
            trade::order::OrderStatusTrade::Initial => {
                unimplemented!("we don't expose initial state to the app")
            }
            trade::order::OrderStatusTrade::Failed => {
                unimplemented!("we don't expose the failed state to the app")
            }
        }
    }
}

#[frb]
#[derive(Debug, Clone, Copy)]
pub enum OrderNotificationType {
    New,
    Update,
}

#[frb]
#[derive(Debug, Clone)]
pub struct OrderNotification {
    pub id: String,
    pub notification_type: OrderNotificationType,
}
