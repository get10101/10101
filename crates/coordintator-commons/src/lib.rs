use bdk::bitcoin::secp256k1::PublicKey;
use bdk::bitcoin::XOnlyPublicKey;
use serde::Deserialize;
use serde::Serialize;
use time::OffsetDateTime;
use trade::ContractSymbol;
use trade::Direction;
use uuid::Uuid;

/// The trade parameters defining the trade execution
///
/// Emitted by the orderbook when a match is found.
/// Both trading parties will receive trade params and then request trade execution with said trade
/// parameters from the coordinator.
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

    /// The direction of the trade
    ///
    /// The direction from the point of view of the trader.
    pub direction: Direction,
}
