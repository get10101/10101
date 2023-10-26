use anyhow::Result;
use bitcoin::Address;
use bitcoin::Amount;
use bitcoin::OutPoint;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use secp256k1::Message as SecpMessage;
use secp256k1::PublicKey;
use secp256k1::XOnlyPublicKey;
use serde::Deserialize;
use serde::Serialize;
use sha2::digest::FixedOutput;
use sha2::Digest;
use sha2::Sha256;
use std::fmt::Display;
use time::OffsetDateTime;
use tokio_tungstenite::tungstenite;
use trade::ContractSymbol;
use trade::Direction;
use uuid::Uuid;

mod order_matching_fee;
mod price;

pub use crate::order_matching_fee::order_matching_fee_taker;
pub use crate::price::best_current_price;
pub use crate::price::Price;
pub use crate::price::Prices;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum OrderState {
    Open,
    Matched,
    Taken,
    Failed,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum OrderReason {
    Manual,
    Expired,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Order {
    pub id: Uuid,
    #[serde(with = "rust_decimal::serde::float")]
    pub price: Decimal,
    pub leverage: f32,
    pub contract_symbol: ContractSymbol,
    pub trader_id: PublicKey,
    pub direction: Direction,
    #[serde(with = "rust_decimal::serde::float")]
    pub quantity: Decimal,
    pub order_type: OrderType,
    #[serde(with = "time::serde::rfc3339")]
    pub timestamp: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub expiry: OffsetDateTime,
    pub order_state: OrderState,
    pub order_reason: OrderReason,
    pub stable: bool,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Signature {
    pub pubkey: PublicKey,
    pub signature: secp256k1::ecdsa::Signature,
}

pub fn create_sign_message() -> SecpMessage {
    let sign_message = "Hello it's me Mario".to_string();
    let hashed_message = Sha256::new().chain_update(sign_message).finalize_fixed();

    let msg = SecpMessage::from_slice(hashed_message.as_slice())
        .expect("The message is static, hence this should never happen");
    msg
}

#[derive(Serialize, Deserialize, Clone)]
pub struct NewOrder {
    pub id: Uuid,
    pub contract_symbol: ContractSymbol,
    #[serde(with = "rust_decimal::serde::float")]
    pub price: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub quantity: Decimal,
    pub trader_id: PublicKey,
    pub direction: Direction,
    pub leverage: f32,
    pub order_type: OrderType,
    pub expiry: OffsetDateTime,
    pub stable: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum OrderType {
    #[allow(dead_code)]
    Market,
    Limit,
}

#[derive(Deserialize)]
pub struct OrderResponse {
    pub id: Uuid,
    #[serde(with = "rust_decimal::serde::float")]
    pub price: Decimal,
    pub trader_id: PublicKey,
    pub direction: Direction,
    #[serde(with = "rust_decimal::serde::float")]
    pub quantity: Decimal,
    pub order_type: OrderType,
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

pub type ChannelId = [u8; 32];

// TODO(holzeis): The message enum should not be in the orderbook-commons crate as it also contains
// coordinator messages. We should move all common crates into a single one.
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
        outpoint: OutPoint,
    },
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

/// A match for an order
///
/// The match defines the execution price and the quantity to be used of the order with the
/// corresponding order id.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Match {
    /// The id of the match
    pub id: Uuid,

    /// The id of the matched order defined by the orderbook
    ///
    /// The identifier of the order as defined by the orderbook.
    pub order_id: Uuid,

    /// The quantity of the matched order to be used
    ///
    /// This might be the complete quantity of the matched order, or a fraction.
    #[serde(with = "rust_decimal::serde::float")]
    pub quantity: Decimal,

    /// Pubkey of the node which order was matched
    pub pubkey: PublicKey,

    /// The execution price as defined by the orderbook
    ///
    /// The trade is to be executed at this price.
    #[serde(with = "rust_decimal::serde::float")]
    pub execution_price: Decimal,
}

impl From<Matches> for Match {
    fn from(value: Matches) -> Self {
        Match {
            id: value.id,
            order_id: value.order_id,
            quantity: value.quantity,
            pubkey: value.trader_id,
            execution_price: value.execution_price,
        }
    }
}

/// The match params for one order
///
/// This is emitted by the orderbook to the trader when an order gets filled.
/// This emitted for one of the trader's order, i.e. the `order_id` matches one of the orders that
/// the trader submitted to the orderbook. The matches define how this order was filled.
/// This information is used to request trade execution with the coordinator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilledWith {
    /// The id of the order defined by the orderbook
    ///
    /// The identifier of the order as defined by the orderbook.
    pub order_id: Uuid,

    /// The expiry timestamp of the contract-to-be
    ///
    /// A timestamp that defines when the contract will expire.
    /// The orderbook defines the timestamp so that the systems using the trade params to set up
    /// the trade are aligned on one timestamp. The systems using the trade params should
    /// validate this timestamp against their trade settings. If the expiry timestamp is older
    /// than a defined threshold a system my discard the trade params as outdated.
    ///
    /// The oracle event-id is defined by contract symbol and the expiry timestamp.
    pub expiry_timestamp: OffsetDateTime,

    /// The public key of the oracle to be used
    ///
    /// The orderbook decides this when matching orders.
    /// The oracle_pk is used to define what oracle is to be used in the contract.
    /// This `oracle_pk` must correspond to one `oracle_pk` configured in the dlc-manager.
    /// It is possible to configure multiple oracles in the dlc-manager; this
    /// `oracle_pk` has to match one of them. This allows us to configure the dlc-managers
    /// using two oracles, where one oracles can be used as backup if the other oracle is not
    /// available. Eventually this can be changed to be a list of oracle PKs and a threshold of
    /// how many oracle have to agree on the attestation.
    pub oracle_pk: XOnlyPublicKey,

    /// The matches for the order
    pub matches: Vec<Match>,
}

impl FilledWith {
    pub fn average_execution_price(&self) -> Decimal {
        average_execution_price(self.matches.clone())
    }
}

/// calculates the average execution price for inverse contracts
///
/// The average execution price follows a simple formula:
/// `total_order_quantity / (quantity_trade_0 / execution_price_trade_0 + quantity_trade_1 /
/// execution_price_trade_1 )`
pub fn average_execution_price(matches: Vec<Match>) -> Decimal {
    if matches.len() == 1 {
        return matches.first().expect("to be exactly one").execution_price;
    }
    let sum_quantity = matches
        .iter()
        .fold(Decimal::ZERO, |acc, m| acc + m.quantity);

    let nominal_prices: Decimal = matches.iter().fold(Decimal::ZERO, |acc, m| {
        acc + (m.quantity / m.execution_price)
    });

    sum_quantity / nominal_prices
}

#[derive(Serialize, Deserialize)]
pub struct RouteHintHop {
    pub src_node_id: PublicKey,
    pub short_channel_id: u64,
    pub fees: RoutingFees,
    pub cltv_expiry_delta: u16,
    pub htlc_minimum_msat: Option<u64>,
    pub htlc_maximum_msat: Option<u64>,
}

#[derive(Serialize, Deserialize)]
pub struct RoutingFees {
    pub base_msat: u32,
    pub proportional_millionths: u32,
}

impl From<lightning::routing::router::RouteHintHop> for RouteHintHop {
    fn from(value: lightning::routing::router::RouteHintHop) -> Self {
        Self {
            src_node_id: value.src_node_id,
            short_channel_id: value.short_channel_id,
            fees: value.fees.into(),
            cltv_expiry_delta: value.cltv_expiry_delta,
            htlc_minimum_msat: value.htlc_minimum_msat,
            htlc_maximum_msat: value.htlc_maximum_msat,
        }
    }
}

impl From<lightning::routing::gossip::RoutingFees> for RoutingFees {
    fn from(value: lightning::routing::gossip::RoutingFees) -> Self {
        Self {
            base_msat: value.base_msat,
            proportional_millionths: value.proportional_millionths,
        }
    }
}

impl From<RouteHintHop> for lightning::routing::router::RouteHintHop {
    fn from(value: RouteHintHop) -> Self {
        Self {
            src_node_id: value.src_node_id,
            short_channel_id: value.short_channel_id,
            fees: value.fees.into(),
            cltv_expiry_delta: value.cltv_expiry_delta,
            htlc_minimum_msat: value.htlc_minimum_msat,
            htlc_maximum_msat: value.htlc_maximum_msat,
        }
    }
}

impl From<RoutingFees> for lightning::routing::gossip::RoutingFees {
    fn from(value: RoutingFees) -> Self {
        Self {
            base_msat: value.base_msat,
            proportional_millionths: value.proportional_millionths,
        }
    }
}

pub enum MatchState {
    Pending,
    Filled,
    Failed,
}

pub struct Matches {
    pub id: Uuid,
    pub match_state: MatchState,
    pub order_id: Uuid,
    pub trader_id: PublicKey,
    pub match_order_id: Uuid,
    pub match_trader_id: PublicKey,
    pub execution_price: Decimal,
    pub quantity: Decimal,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[cfg(test)]
mod test {
    use crate::FilledWith;
    use crate::Match;
    use crate::Signature;
    use rust_decimal_macros::dec;
    use secp256k1::PublicKey;
    use secp256k1::SecretKey;
    use secp256k1::XOnlyPublicKey;
    use std::str::FromStr;
    use time::OffsetDateTime;
    use uuid::Uuid;

    fn dummy_public_key() -> PublicKey {
        PublicKey::from_str("02bd998ebd176715fe92b7467cf6b1df8023950a4dd911db4c94dfc89cc9f5a655")
            .unwrap()
    }

    #[test]
    fn test_serialize_signature() {
        let secret_key = SecretKey::from_slice(&[
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            24, 25, 26, 27, 27, 29, 30, 31,
        ])
        .unwrap();
        let sig = Signature {
            pubkey: secret_key.public_key(&secp256k1::Secp256k1::new()),
            signature: "3045022100ddd8e15dea994a3dd98c481d901fb46b7f3624bb25b4210ea10f8a00779c6f0e0220222235da47b1ba293184fa4a91b39999911c08020e069c9f4afa2d81586b23e1".parse().unwrap(),
        };

        let serialized = serde_json::to_string(&sig).unwrap();

        assert_eq!(
            serialized,
            r#"{"pubkey":"02bd998ebd176715fe92b7467cf6b1df8023950a4dd911db4c94dfc89cc9f5a655","signature":"3045022100ddd8e15dea994a3dd98c481d901fb46b7f3624bb25b4210ea10f8a00779c6f0e0220222235da47b1ba293184fa4a91b39999911c08020e069c9f4afa2d81586b23e1"}"#
        );
    }

    #[test]
    fn test_deserialize_signature() {
        let sig = r#"{"pubkey":"02bd998ebd176715fe92b7467cf6b1df8023950a4dd911db4c94dfc89cc9f5a655","signature":"3045022100ddd8e15dea994a3dd98c481d901fb46b7f3624bb25b4210ea10f8a00779c6f0e0220222235da47b1ba293184fa4a91b39999911c08020e069c9f4afa2d81586b23e1"}"#;
        let serialized: Signature = serde_json::from_str(sig).unwrap();

        let signature = Signature {
            pubkey: dummy_public_key(),
            signature: "3045022100ddd8e15dea994a3dd98c481d901fb46b7f3624bb25b4210ea10f8a00779c6f0e0220222235da47b1ba293184fa4a91b39999911c08020e069c9f4afa2d81586b23e1".parse().unwrap(),
        };

        assert_eq!(serialized, signature);
    }

    #[test]
    fn test_average_execution_price() {
        let match_0_quantity = dec!(1000);
        let match_0_price = dec!(10_000);
        let match_1_quantity = dec!(2000);
        let match_1_price = dec!(12_000);
        let filled = FilledWith {
            order_id: Default::default(),
            expiry_timestamp: OffsetDateTime::now_utc(),
            oracle_pk: XOnlyPublicKey::from_str(
                "16f88cf7d21e6c0f46bcbc983a4e3b19726c6c98858cc31c83551a88fde171c0",
            )
            .expect("To be a valid pubkey"),
            matches: vec![
                Match {
                    id: Uuid::new_v4(),
                    order_id: Default::default(),
                    quantity: match_0_quantity,
                    pubkey: dummy_public_key(),
                    execution_price: match_0_price,
                },
                Match {
                    id: Uuid::new_v4(),
                    order_id: Default::default(),
                    quantity: match_1_quantity,
                    pubkey: dummy_public_key(),
                    execution_price: match_1_price,
                },
            ],
        };

        let average_execution_price = filled.average_execution_price();

        assert_eq!(average_execution_price.round_dp(2), dec!(11250.00));
    }
}
