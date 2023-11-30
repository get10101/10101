use crate::order::Order;
use crate::signature::Signature;
use crate::trade::FilledWith;
use anyhow::Result;
use bitcoin::Address;
use bitcoin::Amount;
use bitcoin::OutPoint;
use rust_decimal::Decimal;
use secp256k1::PublicKey;
use serde::Deserialize;
use serde::Serialize;
use std::fmt::Display;
use tokio_tungstenite::tungstenite;
use uuid::Uuid;

pub type ChannelId = [u8; 32];

#[derive(Serialize, Clone, Deserialize, Debug)]
pub enum Message {
    AllOrders(Vec<Order>),
    LimitOrderFilledMatches {
        trader_id: PublicKey,
        matches: Vec<(Uuid, Decimal)>,
    },
    NewOrder(Order),
    DeleteOrder(Uuid),
    Update(Order),
    InvalidAuthentication(String),
    Authenticated,
    Match(FilledWith),
    AsyncMatch {
        order: Order,
        filled_with: FilledWith,
    },
    Rollover(Option<String>),
    CollaborativeRevert {
        channel_id: ChannelId,
        coordinator_address: Address,
        #[serde(with = "bitcoin::util::amount::serde::as_sat")]
        coordinator_amount: Amount,
        #[serde(with = "bitcoin::util::amount::serde::as_sat")]
        trader_amount: Amount,
        #[serde(with = "rust_decimal::serde::float")]
        execution_price: Decimal,
        funding_txo: OutPoint,
    },
}

#[derive(Serialize, Clone, Deserialize, Debug)]
pub enum OrderbookRequest {
    Authenticate {
        fcm_token: Option<String>,
        signature: Signature,
    },
    LimitOrderFilledMatches {
        trader_id: PublicKey,
    },
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
        match self {
            Message::AllOrders(_) => {
                write!(f, "AllOrdere")
            }
            Message::LimitOrderFilledMatches { .. } => {
                write!(f, "LimitOrderFilledMatches")
            }
            Message::NewOrder(_) => {
                write!(f, "NewOrder")
            }
            Message::DeleteOrder(_) => {
                write!(f, "DeleteOrder")
            }
            Message::Update(_) => {
                write!(f, "Update")
            }
            Message::InvalidAuthentication(_) => {
                write!(f, "InvalidAuthentication")
            }
            Message::Authenticated => {
                write!(f, "Authenticated")
            }
            Message::Match(_) => {
                write!(f, "Match")
            }
            Message::AsyncMatch { .. } => {
                write!(f, "AsyncMatch")
            }
            Message::Rollover(_) => {
                write!(f, "Rollover")
            }
            Message::CollaborativeRevert { .. } => {
                write!(f, "CollaborativeRevert")
            }
        }
    }
}
