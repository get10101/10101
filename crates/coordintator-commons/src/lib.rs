use bdk::bitcoin::secp256k1::PublicKey;
use orderbook_commons::FilledWith;
use serde::Deserialize;
use serde::Serialize;
use trade::ContractSymbol;
use trade::Direction;

/// The trade parameters defining the trade execution
///
/// Emitted by the orderbook when a match is found.
/// Both trading parties will receive trade params and then request trade execution with said trade
/// parameters from the coordinator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeParams {
    /// The identity of the trader
    pub pubkey: PublicKey,

    /// The contract symbol for the trade to be set up
    pub contract_symbol: ContractSymbol,

    /// The leverage of the trader
    ///
    /// This has to correspond to our order's leverage.
    pub leverage: f64,

    /// The quantity of the trader
    ///
    /// For the trade set up with the coordinator it is the quantity of the contract.
    /// This quantity may be the complete quantity of an order or a fraction.
    pub quantity: f64,

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
    pub fn weighted_execution_price(&self) -> f64 {
        if self.filled_with.matches.len() == 1 {
            return self
                .filled_with
                .matches
                .first()
                .expect("to be exactly one")
                .execution_price;
        }

        // TODO: Make sure this is correct

        let sum_weighted_execution_price: f64 = self
            .filled_with
            .matches
            .iter()
            .map(|m| m.execution_price * m.quantity)
            .collect::<Vec<f64>>()
            .iter()
            .sum();
        let quantities: f64 = self
            .filled_with
            .matches
            .iter()
            .map(|m| m.quantity)
            .collect::<Vec<f64>>()
            .iter()
            .sum();

        sum_weighted_execution_price / quantities
    }
}
