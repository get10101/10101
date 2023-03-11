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
    Limit { price: f64 },
}

/// State of an order
///
/// Please refer to [`crate::trade::order::OrderStateTrade`]
#[frb]
#[derive(Debug, Clone, Copy)]
pub enum OrderState {
    Open,
    Failed,
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
    pub id: String,
    pub leverage: f64,
    pub quantity: f64,
    pub contract_symbol: ContractSymbol,
    pub direction: Direction,
    pub order_type: Box<OrderType>,
    pub state: OrderState,
    pub execution_price: Option<f64>,
    pub creation_timestamp: i64,
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
            order::OrderState::Filled { execution_price } => Some(execution_price),
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
            order::OrderState::Initial => unimplemented!(
                "don't expose orders that were not submitted into the orderbook to the frontend!"
            ),
            order::OrderState::Rejected => unimplemented!(
                "don't expose orders that were rejected by the orderbook to the frontend!"
            ),
            // We don't expose this state, but treat it as Open in the UI
            order::OrderState::Filling { .. } => OrderState::Open,
        }
    }
}

impl From<NewOrder> for order::Order {
    fn from(value: NewOrder) -> Self {
        order::Order {
            // todo: Added static uuid for testing reasons. Remove once the dlc can be referenced to
            // an order_id
            id: Uuid::parse_str("02f09a3f-1624-3b1d-8409-44eff7708208").unwrap(), //Uuid::new_v4(),
            leverage: value.leverage,
            quantity: value.quantity,
            contract_symbol: value.contract_symbol,
            direction: value.direction,
            order_type: (*value.order_type).into(),
            state: order::OrderState::Open,
            creation_timestamp: OffsetDateTime::now_utc(),
        }
    }
}
