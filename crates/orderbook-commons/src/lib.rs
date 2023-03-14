use rust_decimal::Decimal;
use secp256k1::Message;
use secp256k1::PublicKey;
use secp256k1::XOnlyPublicKey;
use serde::Deserialize;
use serde::Serialize;
use sha2::digest::FixedOutput;
use sha2::Digest;
use sha2::Sha256;
use time::OffsetDateTime;
use trade::Direction;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Order {
    pub id: Uuid,
    #[serde(with = "rust_decimal::serde::float")]
    pub price: Decimal,
    pub trader_id: PublicKey,
    pub taken: bool,
    pub direction: Direction,
    #[serde(with = "rust_decimal::serde::float")]
    pub quantity: Decimal,
    pub order_type: OrderType,
    pub timestamp: OffsetDateTime,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Signature {
    pub pubkey: PublicKey,
    pub signature: secp256k1::ecdsa::Signature,
}

pub fn create_sign_message() -> Message {
    let sign_message = "Hello it's me Mario".to_string();
    let hashed_message = Sha256::new().chain_update(sign_message).finalize_fixed();

    let msg = Message::from_slice(hashed_message.as_slice())
        .expect("The message is static, hence this should never happen");
    msg
}

#[derive(Serialize, Deserialize, Clone)]
pub struct NewOrder {
    #[serde(with = "rust_decimal::serde::float")]
    pub price: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub quantity: Decimal,
    pub trader_id: PublicKey,
    pub direction: Direction,
    pub order_type: OrderType,
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
    pub taken: bool,
    pub direction: Direction,
    #[serde(with = "rust_decimal::serde::float")]
    pub quantity: Decimal,
    pub order_type: OrderType,
}

#[derive(Serialize, Clone, Deserialize, Debug)]
pub enum OrderbookRequest {
    Authenticate(Signature),
}

#[derive(Serialize, Clone, Deserialize, Debug)]
pub enum OrderbookMsg {
    AllOrders(Vec<Order>),
    NewOrder(Order),
    DeleteOrder(Uuid),
    Update(Order),
    InvalidAuthentication(String),
    Authenticated,
    Match(FilledWith),
}

/// A match for an order
///
/// The match defines the execution price and the quantity to be used of the order with the
/// corresponding order id.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Match {
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

#[cfg(test)]
mod test {
    use crate::Signature;
    use secp256k1::PublicKey;
    use secp256k1::SecretKey;
    use std::str::FromStr;

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
            pubkey: PublicKey::from_str("02bd998ebd176715fe92b7467cf6b1df8023950a4dd911db4c94dfc89cc9f5a655").unwrap(),
            signature: "3045022100ddd8e15dea994a3dd98c481d901fb46b7f3624bb25b4210ea10f8a00779c6f0e0220222235da47b1ba293184fa4a91b39999911c08020e069c9f4afa2d81586b23e1".parse().unwrap(),
        };

        assert_eq!(serialized, signature);
    }
}
