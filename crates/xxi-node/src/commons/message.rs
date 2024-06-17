use crate::commons::order::Order;
use crate::commons::signature::Signature;
use crate::commons::FundingRate;
use crate::commons::LiquidityOption;
use crate::commons::NewLimitOrder;
use crate::commons::ReferralStatus;
use crate::FundingFeeEvent;
use anyhow::Result;
use bitcoin::address::NetworkUnchecked;
use bitcoin::Address;
use bitcoin::Amount;
use rust_decimal::Decimal;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::fmt::Display;
use thiserror::Error;
use tokio_tungstenite_wasm as tungstenite;
use uuid::Uuid;

pub type ChannelId = [u8; 32];
pub type DlcChannelId = [u8; 32];

#[derive(Serialize, Clone, Deserialize, Debug)]
pub enum Message {
    AllOrders(Vec<Order>),
    NewOrder(Order),
    DeleteOrder(Uuid),
    Update(Order),
    InvalidAuthentication(String),
    Authenticated(TenTenOneConfig),
    /// Message used to collaboratively revert DLC channels.
    DlcChannelCollaborativeRevert {
        channel_id: DlcChannelId,
        coordinator_address: Address<NetworkUnchecked>,
        #[serde(with = "bitcoin::amount::serde::as_sat")]
        coordinator_amount: Amount,
        #[serde(with = "bitcoin::amount::serde::as_sat")]
        trader_amount: Amount,
        #[serde(with = "rust_decimal::serde::float")]
        execution_price: Decimal,
    },
    TradeError {
        order_id: Uuid,
        error: TradingError,
    },
    LnPaymentReceived {
        r_hash: String,
        #[serde(with = "bitcoin::amount::serde::as_sat")]
        amount: Amount,
    },
    RolloverError {
        error: TradingError,
    },
    FundingFeeEvent(FundingFeeEvent),
    AllFundingFeeEvents(Vec<FundingFeeEvent>),
    NextFundingRate(FundingRate),
}

#[derive(Serialize, Deserialize, Clone, Error, Debug, PartialEq)]
pub enum TradingError {
    #[error("Invalid order: {0}")]
    InvalidOrder(String),
    #[error("No match found: {0}")]
    NoMatchFound(String),
    #[error("{0}")]
    Other(String),
}

impl From<anyhow::Error> for TradingError {
    fn from(value: anyhow::Error) -> Self {
        TradingError::Other(format!("{value:#}"))
    }
}

#[derive(Serialize, Clone, Deserialize, Debug)]
pub struct TenTenOneConfig {
    // The liquidity options for onboarding
    pub liquidity_options: Vec<LiquidityOption>,
    pub min_quantity: u64,
    pub maintenance_margin_rate: f32,
    pub order_matching_fee_rate: f32,
    pub referral_status: ReferralStatus,
    pub max_leverage: u8,
    pub default_coordinator_leverage: u8,
    /// Coordinator leverage according to trader's leverage.
    ///
    /// Key=trader_leverage, value=coordinator_leverage
    pub coordinator_leverages: HashMap<u8, u8>,
}

#[derive(Serialize, Clone, Deserialize, Debug)]
pub enum OrderbookRequest {
    Authenticate {
        fcm_token: Option<String>,
        version: Option<String>,
        os: Option<String>,
        signature: Signature,
    },
    InsertOrder(NewLimitOrder),
    DeleteOrder(Uuid),
}

impl TryFrom<OrderbookRequest> for tungstenite::Message {
    type Error = anyhow::Error;

    fn try_from(request: OrderbookRequest) -> Result<Self> {
        let msg = serde_json::to_string(&request)?;
        Ok(tungstenite::Message::Text(msg))
    }
}

impl Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Message::AllOrders(_) => "AllOrders",
            Message::NewOrder(_) => "NewOrder",
            Message::DeleteOrder(_) => "DeleteOrder",
            Message::Update(_) => "Update",
            Message::InvalidAuthentication(_) => "InvalidAuthentication",
            Message::Authenticated(_) => "Authenticated",
            Message::DlcChannelCollaborativeRevert { .. } => "DlcChannelCollaborativeRevert",
            Message::TradeError { .. } => "TradeError",
            Message::RolloverError { .. } => "RolloverError",
            Message::LnPaymentReceived { .. } => "LnPaymentReceived",
            Message::FundingFeeEvent(_) => "FundingFeeEvent",
            Message::AllFundingFeeEvents(_) => "FundingFeeEvent",
            Message::NextFundingRate(_) => "NextFundingRate",
        };

        f.write_str(s)
    }
}

/// All values are from the perspective of the coordinator
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PositionMessage {
    /// The current position as seen from the coordinator
    CurrentPosition {
        /// if quantity is < 0 then coordinator is short, if > 0, then coordinator is long
        quantity: f32,
        average_entry_price: f32,
    },
    /// A new trade which was executed successfully
    NewTrade {
        /// The coordinator's total position
        ///
        /// if quantity is < 0 then coordinator is short, if > 0, then coordinator is long
        total_quantity: f32,
        /// The average entry price of the total position
        total_average_entry_price: f32,
        /// The quantity of the new trade
        ///
        /// if quantity is < 0 then coordinator is short, if > 0, then coordinator is long
        new_trade_quantity: f32,
        /// The average entry price of the new trade
        new_trade_average_entry_price: f32,
    },
    Authenticated,
    InvalidAuthentication(String),
}

impl TryFrom<PositionMessage> for tungstenite::Message {
    type Error = anyhow::Error;

    fn try_from(request: PositionMessage) -> Result<Self> {
        let msg = serde_json::to_string(&request)?;
        Ok(tungstenite::Message::Text(msg))
    }
}

impl TryFrom<PositionMessageRequest> for tungstenite::Message {
    type Error = anyhow::Error;

    fn try_from(request: PositionMessageRequest) -> Result<Self> {
        let msg = serde_json::to_string(&request)?;
        Ok(tungstenite::Message::Text(msg))
    }
}

#[derive(Serialize, Clone, Deserialize, Debug)]
pub enum PositionMessageRequest {
    Authenticate { signature: Signature },
}
