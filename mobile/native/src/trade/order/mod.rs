use crate::trade::ContractSymbolTrade;
use crate::trade::DirectionTrade;
use uuid::Uuid;

pub mod api;
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
pub enum OrderStateTrade {
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
    /// This is a final state.
    Rejected,

    /// Successfully submit to orderbook
    ///
    /// If the orderbook returns success upon submission.
    /// Transitions:
    /// - Open->Failed (if we fail to set up the trade)
    /// - Open->Filled (if we successfully set up the trade)
    Open,

    /// Failed to set up a trade
    /// In order to reach this state the orderbook must have provided trade params to start trade
    /// execution, and the trade execution failed.
    /// For the MVP there won't be a retry mechanism, so this is treated as a final state.
    /// This is a final state.
    Failed,

    /// Successfully set up trade
    ///
    /// In order to reach this state the orderbook must have provided trade params to start trade
    /// execution, and the trade execution succeeded. This state assumes that a DLC exists, and
    /// the order is reflected in a position. Note that only complete filling is supported,
    /// partial filling not depicted yet.
    /// This is a final state
    Filled {
        /// The execution price that the order was filled with
        execution_price: f64,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct OrderTrade {
    pub id: Uuid,
    pub leverage: f64,
    pub quantity: f64,
    pub contract_symbol: ContractSymbolTrade,
    pub direction: DirectionTrade,
    pub order_type: OrderTypeTrade,
    pub status: OrderStateTrade,
}
