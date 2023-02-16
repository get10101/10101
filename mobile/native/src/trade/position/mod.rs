pub mod handler;
pub mod notifications;

/// Information returned by the Orderbook for executing a trade
pub struct TradeParams {
    // TODO: Define parameters needed to execute a trade
}

pub enum OrderbookEvent {
    /// The orderbook notifies us with a match
    ///
    /// The position manager will manage the trade execution with the TradeInformation.
    CompleteFillWith(TradeParams),
}
