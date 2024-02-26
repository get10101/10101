use anyhow::Result;
use bitcoin::hashes::sha256;
use bitcoin::secp256k1::PublicKey;
use rust_decimal::Decimal;
use secp256k1::ecdsa::Signature;
use secp256k1::Message;
use secp256k1::VerifyOnly;
use serde::Deserialize;
use serde::Serialize;
use time::OffsetDateTime;
use trade::ContractSymbol;
use trade::Direction;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone)]
pub struct NewOrderRequest {
    pub value: NewOrder,
    /// A signature of the sha256 of [`value`]
    pub signature: Signature,
}

impl NewOrderRequest {
    pub fn verify(&self, secp: &secp256k1::Secp256k1<VerifyOnly>) -> Result<()> {
        let message = self.value.message();
        let public_key = self.value.trader_id;
        secp.verify_ecdsa(&message, &self.signature, &public_key)?;

        Ok(())
    }
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

impl NewOrder {
    pub fn message(&self) -> Message {
        let mut vec: Vec<u8> = vec![];
        let mut id = self.id.as_bytes().to_vec();
        let seconds = self.expiry.second();
        let symbol = self.contract_symbol.label();
        let symbol = symbol.as_bytes();
        let order_type = self.order_type.label();
        let order_type = order_type.as_bytes();
        let direction = self.direction.to_string();
        let direction = direction.as_bytes();
        let quantity = self.quantity.to_string();
        let quantity = quantity.as_bytes();
        let price = self.price.to_string();
        let price = price.as_bytes();
        let string = self.leverage.to_string();
        let leverage = string.as_bytes();
        vec.append(&mut id);
        vec.push(seconds);
        vec.append(&mut symbol.to_vec());
        vec.append(&mut order_type.to_vec());
        vec.append(&mut direction.to_vec());
        vec.append(&mut quantity.to_vec());
        vec.append(&mut price.to_vec());
        vec.append(&mut leverage.to_vec());

        Message::from_hashed_data::<sha256::Hash>(vec.as_slice())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum OrderType {
    #[allow(dead_code)]
    Market,
    Limit,
}

impl OrderType {
    pub fn label(self) -> String {
        match self {
            OrderType::Market => "Market",
            OrderType::Limit => "Limit",
        }
        .to_string()
    }
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum OrderState {
    Open,
    Matched,
    Taken,
    Failed,
    Expired,
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

#[cfg(test)]
pub mod tests {
    use crate::NewOrder;
    use crate::NewOrderRequest;
    use crate::OrderType;
    use secp256k1::rand;
    use secp256k1::Secp256k1;
    use secp256k1::SecretKey;
    use secp256k1::SECP256K1;
    use time::OffsetDateTime;
    use trade::ContractSymbol;
    use trade::Direction;

    #[test]
    pub fn round_trip_signature_new_order() {
        let secret_key = SecretKey::new(&mut rand::thread_rng());
        let public_key = secret_key.public_key(SECP256K1);

        let order = NewOrder {
            id: Default::default(),
            contract_symbol: ContractSymbol::BtcUsd,
            price: rust_decimal_macros::dec!(53_000),
            quantity: rust_decimal_macros::dec!(2000),
            trader_id: public_key,
            direction: Direction::Long,
            leverage: 2.0,
            order_type: OrderType::Market,
            expiry: OffsetDateTime::now_utc(),
            stable: false,
        };

        let message = order.message();

        let signature = secret_key.sign_ecdsa(message);
        signature.verify(&message, &public_key).unwrap();
    }

    #[test]
    pub fn parse_new_order_request_from_string_and_verify() {
        let new_order_string = "{\"value\":{\"id\":\"00000000-0000-0000-0000-000000000000\",\"contract_symbol\":\"BtcUsd\",\"price\":53000.0,\"quantity\":2000.0,\"trader_id\":\"02165446faa03b41d7f2e29741c5d5d5a27a3c1667f6a35d6ea03ba7c2d9619e35\",\"direction\":\"Long\",\"leverage\":2.0,\"order_type\":\"Market\",\"expiry\":[2024,53,12,18,24,406906000,0,0,0],\"stable\":false},\"signature\":\"304402203290d4415c230360f43847586bcf68d11b925e1c3011aab89a7c11d99fd3d5fa0220542830b5ec92a1b6e48240ea5205d66306668728402a5058cee014cecce38f40\"}";
        let new_order: NewOrderRequest = serde_json::from_str(new_order_string).unwrap();

        let secp = Secp256k1::verification_only();
        new_order.verify(&secp).unwrap();
    }
}
