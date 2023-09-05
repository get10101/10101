use crate::calculations::calculate_margin;
use crate::commons::reqwest_client;
use crate::config;
use crate::ln_dlc;
use anyhow::anyhow;
use anyhow::Context;
use anyhow::Result;
use coordinator_commons::RegisterParams;
use rust_decimal::Decimal;
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
#[derive(Debug, Clone, Copy)]
pub enum OrderType {
    Market,
    Limit { price: f32 },
}

/// Internal type so we still have Copy on order
#[derive(Debug, Clone, Copy)]
pub enum FailureReason {
    FailedToSetToFilling,
    TradeRequest,
    TradeResponse,
    NodeAccess,
    NoUsableChannel,
    ProposeDlcChannel,
    /// MVP scope: Can only close the order, not reduce or extend
    OrderNotAcceptable,
    TimedOut,
}

#[derive(Debug, Clone, Copy)]
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
    Filling { execution_price: f32 },

    /// The order failed to be filled
    ///
    /// In order to reach this state the orderbook must have provided trade params to start trade
    /// execution, and the trade execution failed; i.e. it did not result in setting up a DLC.
    /// For the MVP there won't be a retry mechanism, so this is treated as a final state.
    /// This is a final state.
    Failed { reason: FailureReason },

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
    },
}

#[derive(Debug, Clone, Copy)]
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
    pub position_expiry_timestamp: OffsetDateTime,
}

impl Order {
    /// This returns the executed price once known
    ///
    /// Logs an error if this function is called on a state where the execution price is not know
    /// yet.
    pub fn execution_price(&self) -> Option<f32> {
        match self.state {
            OrderState::Filling { execution_price } | OrderState::Filled { execution_price } => {
                Some(execution_price)
            }
            _ => {
                tracing::error!("Executed price not known in state {:?}", self.state);
                None
            }
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

impl From<Order> for orderbook_commons::NewOrder {
    fn from(order: Order) -> Self {
        let quantity = Decimal::try_from(order.quantity).expect("to parse into decimal");
        let trader_id = ln_dlc::get_node_info().pubkey;
        orderbook_commons::NewOrder {
            id: order.id,
            // todo: this is left out intentionally as market orders do not set a price. this field
            // should either be an option or differently modelled for a market order.
            price: Decimal::ZERO,
            quantity,
            trader_id,
            direction: order.direction,
            order_type: order.order_type.into(),
            order_expiry: order.order_expiry_timestamp,
            position_expiry: order.position_expiry_timestamp,
        }
    }
}

impl From<OrderType> for orderbook_commons::OrderType {
    fn from(order_type: OrderType) -> Self {
        match order_type {
            OrderType::Market => orderbook_commons::OrderType::Market,
            OrderType::Limit { .. } => orderbook_commons::OrderType::Limit,
        }
    }
}

/// Enroll the user in the beta program
pub async fn register_beta(email: String) -> Result<()> {
    let register = RegisterParams {
        pubkey: ln_dlc::get_node_info().pubkey,
        email: Some(email),
        nostr: None,
    };

    let client = reqwest_client();
    let response = client
        .post(format!(
            "http://{}/api/register",
            config::get_http_endpoint()
        ))
        .json(&register)
        .send()
        .await
        .context("Failed to register beta program with coordinator")?;

    if !response.status().is_success() {
        let response_text = match response.text().await {
            Ok(text) => text,
            Err(err) => {
                format!("could not decode response {err:#}")
            }
        };
        return Err(anyhow!(
            "Could not register email with coordinator: {response_text}"
        ));
    }
    tracing::info!("Registered into beta program successfully");
    Ok(())
}
