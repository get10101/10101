use anyhow::bail;
use bdk::bitcoin::secp256k1::PublicKey;
use rust_decimal::Decimal;
use serde::Deserialize;
use serde::Serialize;
use std::fmt;
use std::fmt::Formatter;
use std::str::FromStr;
use std::time::Duration;
use uuid::Uuid;

pub mod cfd;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NewOrder {
    pub price: Decimal,
    pub quantity: Decimal,
    pub maker_id: String,
    pub direction: Direction,
}

/// A match represents the matched orders of a taker and a maker and will be executed by the
/// coordinator.
///
/// Emitted by the orderbook to the coordinator for execution when a match is found.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchParams {
    /// The taker trade information
    ///
    /// Note, for the MVP we are not supporting partial order filling, hence this is always exactly
    /// one. We will need a Vector here once we support partial order filling.
    pub taker: Trade,

    /// The maker trade information
    ///
    /// Note, for the MVP we are not supporting partial order filling, hence this is always exactly
    /// one. We will need a Vector here once we support partial order filling.
    pub maker: Trade,

    /// The match params of the trade unspecific to the individual trader
    pub params: Match,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeParams {
    /// Our identity
    pub pubkey: PublicKey,
    /// Identity of the trade's counterparty
    ///
    /// The identity of the trading party that eas matched to our order by the orderbook.
    pub pubkey_counterparty: PublicKey,

    /// The id of the order
    ///
    /// The order has to be identifiable by the client when returned from the orderbook, so the
    /// client is in charge of creating this ID and passing it to the orderbook.
    pub order_id: Uuid,

    /// The orderbook id of the counterparty order
    ///
    /// The orderbook id of the order that was matched with ours.
    /// This can be used by the coordinator to make sure the trade is set up correctly.
    pub order_id_counterparty: String,

    /// The contract symbol for the order
    pub contract_symbol: ContractSymbol,

    /// Our leverage
    ///
    /// This has to correspond to our order's leverage.
    pub leverage: f64,

    /// The leverage of the order that was matched
    ///
    /// This is the leverage of the counterparty.
    /// This can be used by the coordinator to make sure the trade is set up correctly.
    pub leverage_counterparty: f64,

    /// The quantity to be used
    ///
    /// This quantity may be the complete amount of either order or a fraction.
    pub quantity: f64,

    /// The execution price as defined by the orderbook
    ///
    /// The trade is to be executed at this price.
    pub execution_price: f64,

    /// The expiry of the contract-to-be
    ///
    /// A duration that defines how long the contract is meant to be valid.
    /// The coordinator calculates the maturity timestamp based on the current time and the expiry.
    pub expiry: Duration,

    /// The public key of the oracle to be used
    ///
    /// The orderbook decides this when matching orders.
    pub oracle_pk: XOnlyPublicKey,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum ContractSymbol {
    BtcUsd,
}

impl ContractSymbol {
    pub fn label(self) -> String {
        match self {
            ContractSymbol::BtcUsd => "btcusd".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Direction {
    Long,
    Short,
}

impl FromStr for ContractSymbol {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_lowercase().as_str() {
            "btcusd" => Ok(ContractSymbol::BtcUsd),
            // BitMEX representation
            "xbtusd" => Ok(ContractSymbol::BtcUsd),
            unknown => bail!("Unknown contract symbol {unknown}"),
        }
    }
}

impl fmt::Display for ContractSymbol {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let symbol = match self {
            ContractSymbol::BtcUsd => "btcusd",
        };
        symbol.to_string().fmt(f)
    }
}

#[cfg(test)]
pub mod tests {
    use crate::ContractSymbol;
    use std::str::FromStr;

    #[test]
    pub fn contract_symbol_from_str() {
        assert_eq!(
            ContractSymbol::from_str("btcusd").unwrap(),
            ContractSymbol::BtcUsd
        );
        assert_eq!(
            ContractSymbol::from_str("BTCUSD").unwrap(),
            ContractSymbol::BtcUsd
        );
        assert_eq!(
            ContractSymbol::from_str("xbtusd").unwrap(),
            ContractSymbol::BtcUsd
        );
        assert!(ContractSymbol::from_str("dogeusd").is_err());
    }
}
