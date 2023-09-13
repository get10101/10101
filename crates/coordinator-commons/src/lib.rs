use bdk::bitcoin::secp256k1::PublicKey;
use orderbook_commons::FilledWith;
use rust_decimal::Decimal;
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
}

/// Registration details for enrolling into the beta program
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterParams {
    pub pubkey: PublicKey,
    pub email: Option<String>,
    pub nostr: Option<String>,
}

impl RegisterParams {
    pub fn is_valid(&self) -> bool {
        self.email.is_some() || self.nostr.is_some()
    }
}

/// LSP channel details
#[derive(Serialize, Deserialize)]
pub struct LspConfig {
    /// The maximum size a new channel may have
    pub max_channel_value_satoshi: u64,

    /// The fee rate to be used for the DLC contracts in sats/vbyte
    pub contract_tx_fee_rate: u64,
}
/// FCM token update parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUpdateParams {
    pub pubkey: String,
    pub fcm_token: String,
}
