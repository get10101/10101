use crate::trade::position::TradeParams;
use anyhow::Result;

/// Execute a trade resulting in a DLC
pub fn handle_trade(_trade_information: TradeParams) -> Result<()> {
    // TODO:

    // the coordinator client
    // pseudocode:

    // let coordinatorClient  = get_coordinator_client();

    // Executes the protocol with the coordinator
    // pseudocode:

    // let dlc = match coordinatorClient.trade(trade_information).await {
    //     Ok(dlc) => dlc,
    //     Err(e) => {
    //         // TODO: update order to "failed" for now; as long as we don't have partial fills
    // that's fine :)         // early return without position creation
    //         return;
    //     }
    // };

    // TODO: update oder to filled
    // TODO: send out oder update notification

    // TODO: Store the DLC as position in database in relation to the order(s) that define it;
    // One position has multiple orders (has multiple trades with partial filling); trades are out
    // of scope with complete filling A position can be either
    //  - created: we don't have a position yet and a long/short position is being filled
    //  - closed: we already have a long/short position and a short/long position is filled that is
    //    voids the existing order
    //  - extended:we already have a long/short position and a long/short order is being filled.
    //  - reduced: we already have a long/short position and a short/long order is being filled.
    //
    // Preferably we update the position values (that are not dependent on the price) when a
    // position is changed by an order; and aggregate and calculate all the time.

    // TODO: Send out new position notification
    // TODO: -> mock this so we can see position in the UI

    unimplemented!()
}
