use bitcoin::secp256k1::PublicKey;
use bitcoin::secp256k1::XOnlyPublicKey;
use bitcoin::Amount;
use rust_decimal::Decimal;
use serde::Deserialize;
use serde::Serialize;
use time::OffsetDateTime;
use trade::ContractSymbol;
use trade::Direction;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TradeAndChannelParams {
    pub trade_params: TradeParams,
    #[serde(with = "bitcoin::amount::serde::as_sat::opt")]
    pub trader_reserve: Option<Amount>,
    #[serde(with = "bitcoin::amount::serde::as_sat::opt")]
    pub coordinator_reserve: Option<Amount>,
}

/// The trade parameters defining the trade execution.
///
/// Emitted by the orderbook when a match is found.
///
/// Both trading parties will receive trade params and then request trade execution with said trade
/// parameters from the coordinator.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TradeParams {
    /// The identity of the trader
    pub pubkey: PublicKey,

    /// The contract symbol for the trade to be set up
    pub contract_symbol: ContractSymbol,

    /// The leverage of the trader
    ///
    /// This has to correspond to our order's leverage.
    pub leverage: f32,

    /// The quantity of the trader
    ///
    /// For the trade set up with the coordinator it is the quantity of the contract.
    /// This quantity may be the complete quantity of an order or a fraction.
    pub quantity: f32,

    /// The direction of the trader
    ///
    /// The direction from the point of view of the trader.
    /// The coordinator takes the counter-position when setting up the trade.
    pub direction: Direction,

    /// The filling information from the orderbook
    ///
    /// This is used by the coordinator to be able to make sure both trading parties are acting.
    /// The `quantity` has to match the cummed up quantities of the matches in `filled_with`.
    pub filled_with: FilledWith,
}

impl TradeParams {
    pub fn average_execution_price(&self) -> Decimal {
        self.filled_with.average_execution_price()
    }

    pub fn order_matching_fee(&self) -> Amount {
        self.filled_with.order_matching_fee()
    }
}

/// A match for an order
///
/// The match defines the execution price and the quantity to be used of the order with the
/// corresponding order id.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

    #[serde(with = "bitcoin::amount::serde::as_sat")]
    pub matching_fee: Amount,
}

impl From<Matches> for Match {
    fn from(value: Matches) -> Self {
        Match {
            id: value.id,
            order_id: value.order_id,
            quantity: value.quantity,
            pubkey: value.trader_id,
            execution_price: value.execution_price,
            matching_fee: value.matching_fee,
        }
    }
}

/// The match params for one order
///
/// This is emitted by the orderbook to the trader when an order gets filled.
/// This emitted for one of the trader's order, i.e. the `order_id` matches one of the orders that
/// the trader submitted to the orderbook. The matches define how this order was filled.
/// This information is used to request trade execution with the coordinator.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
    pub fn order_matching_fee(&self) -> Amount {
        self.matches.iter().map(|m| m.matching_fee).sum()
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
    pub matching_fee: Amount,
}

#[cfg(test)]
mod test {
    fn dummy_public_key() -> PublicKey {
        PublicKey::from_str("02bd998ebd176715fe92b7467cf6b1df8023950a4dd911db4c94dfc89cc9f5a655")
            .unwrap()
    }

    use crate::trade::FilledWith;
    use crate::trade::Match;
    use bitcoin::secp256k1::PublicKey;
    use bitcoin::secp256k1::XOnlyPublicKey;
    use bitcoin::Amount;
    use rust_decimal_macros::dec;
    use std::str::FromStr;
    use time::OffsetDateTime;
    use uuid::Uuid;

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
                    matching_fee: Amount::from_sat(1000),
                },
                Match {
                    id: Uuid::new_v4(),
                    order_id: Default::default(),
                    quantity: match_1_quantity,
                    pubkey: dummy_public_key(),
                    execution_price: match_1_price,
                    matching_fee: Amount::from_sat(1000),
                },
            ],
        };

        let average_execution_price = filled.average_execution_price();

        assert_eq!(average_execution_price.round_dp(2), dec!(11250.00));
    }
}
