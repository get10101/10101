use crate::trade::order;
use flutter_rust_bridge::frb;
use time::OffsetDateTime;
use trade::ContractSymbol;
use trade::Direction;
use uuid::Uuid;

#[frb]
#[derive(Debug, Clone, Copy)]
pub enum OrderType {
    Market,
    Limit { price: f32 },
}

/// State of an order
///
/// Please refer to [`crate::trade::order::OrderState`]
#[frb]
#[derive(Debug, Clone, Copy)]
pub enum OrderState {
    Open,
    Filling,
    Filled,
    Failed,
    Rejected,
}

#[frb]
#[derive(Debug, Clone, Copy)]
pub enum OrderReason {
    Manual,
    Expired,
    Liquidated,
}

#[frb]
#[derive(Debug, Clone)]
pub enum FailureReason {
    FailedToSetToFilling,
    TradeRequest,
    TradeResponse(String),
    NodeAccess,
    NoUsableChannel,
    CollabRevert,
    OrderNotAcceptable,
    TimedOut,
    InvalidDlcOffer,
    OrderRejected(String),
    Unknown,
}

#[frb]
#[derive(Debug, Clone)]
pub struct NewOrder {
    #[frb(non_final)]
    pub leverage: f32,
    #[frb(non_final)]
    pub quantity: f32,
    #[frb(non_final)]
    pub contract_symbol: ContractSymbol,
    #[frb(non_final)]
    pub direction: Direction,
    // Box needed for complex enum, otherwise generated Rust code complains about Default impl
    // missing
    #[frb(non_final)]
    pub order_type: Box<OrderType>,
    #[frb(non_final)]
    pub stable: bool,
}

#[frb]
#[derive(Debug, Clone)]
pub struct Order {
    pub id: String,
    pub leverage: f32,
    pub quantity: f32,
    pub contract_symbol: ContractSymbol,
    pub direction: Direction,
    pub order_type: Box<OrderType>,
    pub state: OrderState,
    pub execution_price: Option<f32>,
    pub creation_timestamp: i64,
    pub order_expiry_timestamp: i64,
    pub reason: OrderReason,
    pub failure_reason: Option<FailureReason>,
}

impl From<order::OrderType> for OrderType {
    fn from(value: order::OrderType) -> Self {
        match value {
            order::OrderType::Market => OrderType::Market,
            order::OrderType::Limit { price } => OrderType::Limit { price },
        }
    }
}

impl From<order::Order> for Order {
    fn from(value: order::Order) -> Self {
        let execution_price = match value.state {
            order::OrderState::Filled {
                execution_price, ..
            } => Some(execution_price),
            _ => None,
        };

        Order {
            id: value.id.to_string(),
            leverage: value.leverage,
            quantity: value.quantity,
            contract_symbol: value.contract_symbol,
            direction: value.direction,
            order_type: Box::new(value.order_type.into()),
            state: value.state.into(),
            execution_price,
            creation_timestamp: value.creation_timestamp.unix_timestamp(),
            order_expiry_timestamp: value.order_expiry_timestamp.unix_timestamp(),
            reason: value.reason.into(),
            failure_reason: value.failure_reason.map(|reason| reason.into()),
        }
    }
}

impl From<order::OrderReason> for OrderReason {
    fn from(value: order::OrderReason) -> Self {
        match value {
            order::OrderReason::Manual => OrderReason::Manual,
            order::OrderReason::Expired => OrderReason::Expired,
            order::OrderReason::CoordinatorLiquidated => OrderReason::Liquidated,
            order::OrderReason::TraderLiquidated => OrderReason::Liquidated,
        }
    }
}

impl From<order::FailureReason> for FailureReason {
    fn from(value: order::FailureReason) -> Self {
        match value {
            order::FailureReason::FailedToSetToFilling => FailureReason::FailedToSetToFilling,
            order::FailureReason::TradeRequest => FailureReason::TradeRequest,
            order::FailureReason::TradeResponse(details) => FailureReason::TradeResponse(details),
            order::FailureReason::OrderNotAcceptable => FailureReason::OrderNotAcceptable,
            order::FailureReason::TimedOut => FailureReason::TimedOut,
            order::FailureReason::InvalidDlcOffer(_) => FailureReason::InvalidDlcOffer,
            order::FailureReason::OrderRejected(reason) => FailureReason::OrderRejected(reason),
            order::FailureReason::CollabRevert => FailureReason::CollabRevert,
            order::FailureReason::Unknown => FailureReason::Unknown,
        }
    }
}

impl From<OrderType> for order::OrderType {
    fn from(value: OrderType) -> Self {
        match value {
            OrderType::Market => order::OrderType::Market,
            OrderType::Limit { price } => order::OrderType::Limit { price },
        }
    }
}

impl From<order::OrderState> for OrderState {
    fn from(value: order::OrderState) -> Self {
        match value {
            order::OrderState::Open => OrderState::Open,
            order::OrderState::Filled { .. } => OrderState::Filled,
            order::OrderState::Failed { .. } => OrderState::Failed,
            order::OrderState::Rejected => OrderState::Rejected,
            order::OrderState::Filling { .. } => OrderState::Filling,
            order::OrderState::Initial => unimplemented!(
                "don't expose orders that were not submitted into the orderbook to the frontend!"
            ),
        }
    }
}

impl From<NewOrder> for order::Order {
    fn from(value: NewOrder) -> Self {
        order::Order {
            id: Uuid::new_v4(),
            leverage: value.leverage,
            quantity: value.quantity,
            contract_symbol: value.contract_symbol,
            direction: value.direction,
            order_type: (*value.order_type).into(),
            state: order::OrderState::Initial,
            creation_timestamp: OffsetDateTime::now_utc(),
            // We do not support setting order expiry from the frontend for now
            order_expiry_timestamp: OffsetDateTime::now_utc() + time::Duration::minutes(1),
            reason: order::OrderReason::Manual,
            stable: value.stable,
            failure_reason: None,
        }
    }
}
