use bitcoin::Amount;
use bitcoin::SignedAmount;
use rust_decimal::Decimal;
use time::OffsetDateTime;
use trade::ContractSymbol;
use trade::Direction;
use uuid::Uuid;

pub mod order;
pub mod position;
pub mod users;

/// A trade is an event that moves funds between the DLC channel collateral reserve and a DLC
/// channel.
///
/// Every trade is associated with a single market order, but an order can be associated with
/// multiple trades.
///
/// If an order changes the direction of the underlying position, it must be split into _two_
/// trades: one to close the original position and another one to open the new position in the
/// opposite direction. We do so to keep the model as simple as possible.
#[derive(Debug, Clone, PartialEq)]
pub struct Trade {
    /// The executed order which resulted in this trade.
    pub order_id: Uuid,
    pub contract_symbol: ContractSymbol,
    pub contracts: Decimal,
    /// Direction of the associated order.
    pub direction: Direction,
    /// How many coins were moved between the DLC channel collateral reserve and the DLC.
    ///
    /// A positive value indicates that the money moved out of the reserve; a negative value
    /// indicates that the money moved into the reserve.
    pub trade_cost: SignedAmount,
    pub fee: Amount,
    /// If a position was reduced or closed because of this trade, how profitable it was.
    ///
    /// Set to [`None`] if the position was extended.
    pub pnl: Option<SignedAmount>,
    /// The price at which the associated order was executed.
    pub price: Decimal,
    pub timestamp: OffsetDateTime,
}
