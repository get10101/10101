use crate::api_model::ContractSymbol;
use crate::api_model::Direction;
use crate::trade::order::OrderStatusTrade;
use crate::trade::order::OrderTrade;
use crate::trade::order::OrderTypeTrade;
use flutter_rust_bridge::frb;
use uuid::Uuid;

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

impl From<OrderType> for OrderTypeTrade {
    fn from(value: OrderType) -> Self {
        match value {
            OrderType::Market => OrderTypeTrade::Market,
            OrderType::Limit { price } => OrderTypeTrade::Limit { price },
        }
    }
}

impl From<OrderStatus> for OrderStatusTrade {
    fn from(value: OrderStatus) -> Self {
        match value {
            OrderStatus::Open => OrderStatusTrade::Open,
            OrderStatus::Filled => OrderStatusTrade::Filled,
        }
    }
}

impl From<OrderTrade> for Order {
    fn from(value: OrderTrade) -> Self {
        Order {
            leverage: value.leverage,
            quantity: value.quantity,
            contract_symbol: value.contract_symbol.into(),
            direction: value.direction.into(),
            order_type: Box::new(value.order_type.into()),
            status: value.status.into(),
        }
    }
}

impl From<OrderTypeTrade> for OrderType {
    fn from(value: OrderTypeTrade) -> Self {
        match value {
            OrderTypeTrade::Market => OrderType::Market,
            OrderTypeTrade::Limit { price } => OrderType::Limit { price },
        }
    }
}

impl From<OrderStatusTrade> for OrderStatus {
    fn from(value: OrderStatusTrade) -> Self {
        match value {
            OrderStatusTrade::Open => OrderStatus::Open,
            OrderStatusTrade::Filled => OrderStatus::Filled,
        }
    }
}

impl From<NewOrder> for OrderTrade {
    fn from(value: NewOrder) -> Self {
        OrderTrade {
            id: Uuid::new_v4(),
            leverage: value.leverage,
            quantity: value.quantity,
            contract_symbol: value.contract_symbol.into(),
            direction: value.direction.into(),
            order_type: (*value.order_type).into(),
            status: OrderStatusTrade::Open,
        }
    }
}
