use crate::calculations::calculate_margin;
use crate::ln_dlc;
use bitcoin::Amount;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;
use serde::Serialize;
use time::OffsetDateTime;
use trade::ContractSymbol;
use trade::Direction;
use uuid::Uuid;

pub mod api;
pub mod handler;
mod orderbook_client;

// When naming this the same as `api_model::order::OrderType` the generated code somehow uses
// `trade::OrderType` and contains errors, hence different name is used.
// This is likely a bug in frb.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub enum OrderType {
    Market,
    Limit { price: f32 },
}

/// Internal type so we still have Copy on order
#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum FailureReason {
    /// An error occurred when setting the Order to filling in our DB
    FailedToSetToFilling,
    /// The order failed because we failed sending the trade request
    TradeRequest,
    /// A failure happened during the initial phase of the protocol. I.e. after sending the trade
    /// request
    TradeResponse(String),
    /// The order failed due to collaboratively reverting the position
    CollabRevert,
    /// MVP scope: Can only close the order, not reduce or extend
    OrderNotAcceptable,
    /// The order timed out, i.e. we did not receive a match in time
    TimedOut,
    InvalidDlcOffer(InvalidSubchannelOffer),
    /// The order has been rejected by the orderbook
    OrderRejected(String),
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
pub enum InvalidSubchannelOffer {
    /// Received offer was outdated
    Outdated,
    UndeterminedMaturityDate,
    Unacceptable,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OrderState {
    /// Not submitted to orderbook yet
    ///
    /// In order to be able to track how many failed orders we have we store the order in the
    /// database and update it once the orderbook returns success.
    /// Transitions:
    /// - Initial->Open
    /// - Initial->Rejected
    Initial,

    /// Rejected by the orderbook upon submission
    ///
    /// If the orderbook returns failure upon submission.
    /// Note that we will not be able to query this order from the orderbook again, because it was
    /// rejected upon submission. This is a final state.
    Rejected,

    /// Successfully submit to orderbook
    ///
    /// If the orderbook returns success upon submission.
    /// Transitions:
    /// - Open->Failed (if we fail to set up the trade)
    /// - Open->Filled (if we successfully set up the trade)
    Open,

    /// The orderbook has matched the order and it is being filled
    ///
    /// Once the order is being filled we know the execution price and store it.
    /// Since it's a non-custodial setup filling an order involves setting up a DLC.
    /// This state is set once we receive the TradeParams from the orderbook.
    /// This state covers the complete trade execution until we have a DLC or we run into a failure
    /// scenario. We don't allow re-trying the trade execution; if the app is started and we
    /// detect an order that is in the `Filling` state, we will have to evaluate if there is a DLC
    /// currently being set up. If yes the order remains in `Filling` state, if there is no DLC
    /// currently being set up we move the order into `Failed` state.
    ///
    /// Transitions:
    /// Filling->Filled (if we eventually end up with a DLC)
    /// Filling->Failed (if we experience an error when executing the trade or the DLC manager
    /// reported back failure/rejection)
    Filling {
        execution_price: f32,
        matching_fee: Amount,
    },

    /// The order failed to be filled
    ///
    /// In order to reach this state the orderbook must have provided trade params to start trade
    /// execution, and the trade execution failed; i.e. it did not result in setting up a DLC.
    /// For the MVP there won't be a retry mechanism, so this is treated as a final state.
    /// This is a final state.
    Failed {
        execution_price: Option<f32>,
        reason: FailureReason,
    },

    /// Successfully set up trade
    ///
    /// In order to reach this state the orderbook must have provided trade params to start trade
    /// execution, and the trade execution succeeded. This state assumes that a DLC exists, and
    /// the order is reflected in a position. Note that only complete filling is supported,
    /// partial filling not depicted yet.
    /// This is a final state
    Filled {
        /// The execution price that the order was filled with
        execution_price: f32,
        matching_fee: Amount,
    },
}

impl OrderState {
    pub fn matching_fee(&self) -> Option<Amount> {
        match self {
            OrderState::Initial
            | OrderState::Rejected
            | OrderState::Failed { .. }
            | OrderState::Open => None,
            OrderState::Filling { matching_fee, .. } | OrderState::Filled { matching_fee, .. } => {
                Some(*matching_fee)
            }
        }
    }
    pub fn execution_price(&self) -> Option<f32> {
        match self {
            OrderState::Initial
            | OrderState::Rejected
            | OrderState::Failed { .. }
            | OrderState::Open => None,
            OrderState::Filling {
                execution_price, ..
            }
            | OrderState::Filled {
                execution_price, ..
            } => Some(*execution_price),
        }
    }
    pub fn failure_reason(&self) -> Option<FailureReason> {
        match self {
            OrderState::Initial | OrderState::Rejected | OrderState::Open => None,
            OrderState::Filling { .. } | OrderState::Filled { .. } => None,
            OrderState::Failed { reason, .. } => Some(reason.clone()),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum OrderReason {
    Manual,
    Expired,
    CoordinatorLiquidated,
    TraderLiquidated,
}

impl From<OrderReason> for commons::OrderReason {
    fn from(value: OrderReason) -> Self {
        match value {
            OrderReason::Manual => commons::OrderReason::Manual,
            OrderReason::Expired => commons::OrderReason::Expired,
            OrderReason::CoordinatorLiquidated => commons::OrderReason::CoordinatorLiquidated,
            OrderReason::TraderLiquidated => commons::OrderReason::TraderLiquidated,
        }
    }
}

impl From<commons::OrderReason> for OrderReason {
    fn from(value: commons::OrderReason) -> Self {
        match value {
            commons::OrderReason::Manual => OrderReason::Manual,
            commons::OrderReason::Expired => OrderReason::Expired,
            commons::OrderReason::CoordinatorLiquidated => OrderReason::CoordinatorLiquidated,
            commons::OrderReason::TraderLiquidated => OrderReason::TraderLiquidated,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Order {
    pub id: Uuid,
    pub leverage: f32,
    pub quantity: f32,
    pub contract_symbol: ContractSymbol,
    pub direction: Direction,
    pub order_type: OrderType,
    pub state: OrderState,
    pub creation_timestamp: OffsetDateTime,
    pub order_expiry_timestamp: OffsetDateTime,
    pub reason: OrderReason,
    pub stable: bool,
    // FIXME: Why is this failure_reason duplicated? It's also in the `order_state`?
    pub failure_reason: Option<FailureReason>,
}

impl Order {
    /// This returns the executed price once known
    ///
    /// Logs an error if this function is called on a state where the execution price is not know
    /// yet.
    pub fn execution_price(&self) -> Option<f32> {
        match self.state {
            OrderState::Filling {
                execution_price, ..
            }
            | OrderState::Filled {
                execution_price, ..
            }
            | OrderState::Failed {
                execution_price: Some(execution_price),
                ..
            } => Some(execution_price),
            _ => {
                // TODO: The caller should decide how to handle this. Always logging an error is
                // weird.
                tracing::error!("Executed price not known in state {:?}", self.state);
                None
            }
        }
    }

    /// This returns the matching fee once known
    pub fn matching_fee(&self) -> Option<Amount> {
        match self.state {
            OrderState::Filling { matching_fee, .. } | OrderState::Filled { matching_fee, .. } => {
                Some(matching_fee)
            }
            _ => None,
        }
    }

    /// This returns the trader's margin once known (based on the execution price).
    pub fn trader_margin(&self) -> Option<u64> {
        let opening_price = self.execution_price()?;

        Some(calculate_margin(
            opening_price,
            self.quantity,
            self.leverage,
        ))
    }
}

impl From<Order> for commons::NewMarketOrder {
    fn from(order: Order) -> Self {
        let quantity = Decimal::try_from(order.quantity).expect("to parse into decimal");
        let trader_id = ln_dlc::get_node_pubkey();
        commons::NewMarketOrder {
            id: order.id,
            contract_symbol: order.contract_symbol,
            quantity,
            trader_id,
            direction: order.direction,
            leverage: Decimal::from_f32(order.leverage).expect("to fit into f32"),
            expiry: order.order_expiry_timestamp,
            stable: order.stable,
        }
    }
}

impl From<OrderType> for commons::OrderType {
    fn from(order_type: OrderType) -> Self {
        match order_type {
            OrderType::Market => commons::OrderType::Market,
            OrderType::Limit { .. } => commons::OrderType::Limit,
        }
    }
}
