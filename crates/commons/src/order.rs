use anyhow::Result;
use bitcoin::hashes::sha256;
use bitcoin::secp256k1::PublicKey;
use bitcoin::Amount;
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
    pub channel_opening_params: Option<ChannelOpeningParams>,
}

impl NewOrderRequest {
    pub fn verify(&self, secp: &secp256k1::Secp256k1<VerifyOnly>) -> Result<()> {
        let message = self.value.message();
        let public_key = self.value.trader_id();
        secp.verify_ecdsa(&message, &self.signature, &public_key)?;

        Ok(())
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub enum NewOrder {
    Market(NewMarketOrder),
    Limit(NewLimitOrder),
}

impl NewOrder {
    pub fn message(&self) -> Message {
        match self {
            NewOrder::Market(o) => o.message(),
            NewOrder::Limit(o) => o.message(),
        }
    }

    pub fn trader_id(&self) -> PublicKey {
        match self {
            NewOrder::Market(o) => o.trader_id,
            NewOrder::Limit(o) => o.trader_id,
        }
    }

    pub fn id(&self) -> Uuid {
        match self {
            NewOrder::Market(o) => o.id,
            NewOrder::Limit(o) => o.id,
        }
    }

    pub fn direction(&self) -> Direction {
        match self {
            NewOrder::Market(o) => o.direction,
            NewOrder::Limit(o) => o.direction,
        }
    }

    pub fn price(&self) -> String {
        match self {
            NewOrder::Market(_) => "Market".to_string(),
            NewOrder::Limit(o) => o.price.to_string(),
        }
    }

    pub fn order_type(&self) -> String {
        match self {
            NewOrder::Market(_) => "Market",
            NewOrder::Limit(_) => "Limit",
        }
        .to_string()
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct NewMarketOrder {
    pub id: Uuid,
    pub contract_symbol: ContractSymbol,
    #[serde(with = "rust_decimal::serde::float")]
    pub quantity: Decimal,
    pub trader_id: PublicKey,
    pub direction: Direction,
    #[serde(with = "rust_decimal::serde::float")]
    pub leverage: Decimal,
    #[serde(with = "time::serde::timestamp")]
    pub expiry: OffsetDateTime,
    pub stable: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct NewLimitOrder {
    pub id: Uuid,
    pub contract_symbol: ContractSymbol,
    #[serde(with = "rust_decimal::serde::float")]
    pub price: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub quantity: Decimal,
    pub trader_id: PublicKey,
    pub direction: Direction,
    #[serde(with = "rust_decimal::serde::float")]
    pub leverage: Decimal,
    #[serde(with = "time::serde::timestamp")]
    pub expiry: OffsetDateTime,
    pub stable: bool,
}

impl NewLimitOrder {
    pub fn message(&self) -> Message {
        let mut vec: Vec<u8> = vec![];
        let mut id = self.id.as_bytes().to_vec();
        let unix_timestamp = self.expiry.unix_timestamp();
        let mut seconds = unix_timestamp.to_le_bytes().to_vec();

        let symbol = self.contract_symbol.label();
        let symbol = symbol.as_bytes();
        let direction = self.direction.to_string();
        let direction = direction.as_bytes();
        let quantity = format!("{:.2}", self.quantity);
        let quantity = quantity.as_bytes();
        let price = format!("{:.2}", self.price);
        let price = price.as_bytes();
        let leverage = format!("{:.2}", self.leverage);
        let leverage = leverage.as_bytes();

        vec.append(&mut id);
        vec.append(&mut seconds);
        vec.append(&mut symbol.to_vec());
        vec.append(&mut direction.to_vec());
        vec.append(&mut quantity.to_vec());
        vec.append(&mut price.to_vec());
        vec.append(&mut leverage.to_vec());

        Message::from_hashed_data::<sha256::Hash>(vec.as_slice())
    }
}

impl NewMarketOrder {
    pub fn message(&self) -> Message {
        let mut vec: Vec<u8> = vec![];
        let mut id = self.id.as_bytes().to_vec();
        let unix_timestamp = self.expiry.unix_timestamp();
        let mut seconds = unix_timestamp.to_le_bytes().to_vec();

        let symbol = self.contract_symbol.label();
        let symbol = symbol.as_bytes();
        let direction = self.direction.to_string();
        let direction = direction.as_bytes();
        let quantity = format!("{:.2}", self.quantity);
        let quantity = quantity.as_bytes();
        let leverage = format!("{:.2}", self.leverage);
        let leverage = leverage.as_bytes();

        vec.append(&mut id);
        vec.append(&mut seconds);
        vec.append(&mut symbol.to_vec());
        vec.append(&mut direction.to_vec());
        vec.append(&mut quantity.to_vec());
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum OrderState {
    Open,
    Matched,
    Taken,
    Failed,
    Expired,
    Deleted,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum OrderReason {
    Manual,
    Expired,
    CoordinatorLiquidated,
    TraderLiquidated,
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

/// Extra information required to open a DLC channel, independent of the [`TradeParams`] associated
/// with the filled order.
///
/// [`TradeParams`]: commons::TradeParams
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct ChannelOpeningParams {
    #[serde(with = "bitcoin::amount::serde::as_sat")]
    pub trader_reserve: Amount,
    #[serde(with = "bitcoin::amount::serde::as_sat")]
    pub coordinator_reserve: Amount,
}

#[cfg(test)]
pub mod tests {
    use crate::NewLimitOrder;
    use crate::NewOrder;
    use crate::NewOrderRequest;
    use secp256k1::rand;
    use secp256k1::Secp256k1;
    use secp256k1::SecretKey;
    use secp256k1::SECP256K1;
    use std::str::FromStr;
    use time::ext::NumericalDuration;
    use time::OffsetDateTime;
    use trade::ContractSymbol;
    use trade::Direction;
    use uuid::Uuid;

    #[test]
    pub fn round_trip_signature_new_order() {
        let secret_key = SecretKey::new(&mut rand::thread_rng());
        let public_key = secret_key.public_key(SECP256K1);

        let order = NewLimitOrder {
            id: Default::default(),
            contract_symbol: ContractSymbol::BtcUsd,
            price: rust_decimal_macros::dec!(53_000),
            quantity: rust_decimal_macros::dec!(2000),
            trader_id: public_key,
            direction: Direction::Long,
            leverage: rust_decimal_macros::dec!(2.0),
            expiry: OffsetDateTime::now_utc(),
            stable: false,
        };

        let message = order.message();

        let signature = secret_key.sign_ecdsa(message);
        signature.verify(&message, &public_key).unwrap();
    }

    #[test]
    pub fn round_trip_order_signature_verification() {
        // setup
        let secret_key =
            SecretKey::from_str("01010101010101010001020304050607ffff0000ffff00006363636363636363")
                .unwrap();
        let public_key = secret_key.public_key(SECP256K1);

        let original_order = NewLimitOrder {
            id: Uuid::from_str("67e5504410b1426f9247bb680e5fe0c8").unwrap(),
            contract_symbol: ContractSymbol::BtcUsd,
            price: rust_decimal_macros::dec!(53_000),
            quantity: rust_decimal_macros::dec!(2000),
            trader_id: public_key,
            direction: Direction::Long,
            leverage: rust_decimal_macros::dec!(2.0),
            // Note: the last 5 is too much as it does not get serialized
            expiry: OffsetDateTime::UNIX_EPOCH + 1.1010101015.seconds(),
            stable: false,
        };

        let message = original_order.clone().message();

        let signature = secret_key.sign_ecdsa(message);
        signature.verify(&message, &public_key).unwrap();

        let original_request = NewOrderRequest {
            value: NewOrder::Limit(original_order),
            signature,
            channel_opening_params: None,
        };

        let original_serialized_request = serde_json::to_string(&original_request).unwrap();

        let serialized_msg = "{\"value\":{\"Limit\":{\"id\":\"67e55044-10b1-426f-9247-bb680e5fe0c8\",\"contract_symbol\":\"BtcUsd\",\"price\":53000.0,\"quantity\":2000.0,\"trader_id\":\"0218845781f631c48f1c9709e23092067d06837f30aa0cd0544ac887fe91ddd166\",\"direction\":\"Long\",\"leverage\":2.0,\"expiry\":1,\"stable\":false}},\"signature\":\"304402205024fd6aea64c02155bdc063cf9168d9cd24fc6d54d3da0db645372828df210e022062323c30a88b60ef647d6740a01ac38fccc7f306f1c380bd92715d8b2e39adb9\",\"channel_opening_params\":null}";

        // replace the signature with the one from above to have the same string
        let serialized_msg =
            serialized_msg.replace("SIGNATURE_PLACEHOLDER", signature.to_string().as_str());

        // act

        let parsed_request: NewOrderRequest =
            serde_json::from_str(serialized_msg.as_str()).unwrap();

        // assert

        // ensure that the two strings are the same, besides the signature (which has a random
        // factor)
        assert_eq!(original_serialized_request, serialized_msg);

        assert_eq!(
            original_request.value.message(),
            parsed_request.value.message()
        );

        // Below would also fail but we don't even get there yet
        let secp = Secp256k1::verification_only();
        parsed_request.verify(&secp).unwrap();
    }
}
