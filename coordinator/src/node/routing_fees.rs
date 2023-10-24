use crate::db;
use crate::node::Node;
use crate::routing_fee::models::NewRoutingFee;
use lightning::events::Event;

/// Save the routing fee in the database upon `PaymentForwarded` event
///
/// Only takes regular routing fees into account. This function does not handle force-close
/// scenarios where the `fee_earned_msat` is set to `None`.
pub fn handle(node: Node, event: Option<Event>) {
    //
    if let Some(Event::PaymentForwarded {
        fee_earned_msat: Some(fee_earned_msat),
        prev_channel_id,
        next_channel_id,
        ..
    }) = event
    {
        tokio::task::spawn_blocking(move || {
            let mut conn = match node.pool.get() {
                Ok(conn) => conn,
                Err(e) => {
                    tracing::error!("Failed to connect to database during node event post processing event: {e:#}");
                    return;
                }
            };

            if let Err(e) = db::routing_fees::insert(
                NewRoutingFee {
                    amount_msats: fee_earned_msat,
                    prev_channel_id,
                    next_channel_id,
                },
                &mut conn,
            ) {
                tracing::error!(%fee_earned_msat, "Failed to insert routing fee into database: {e:#}");
            }
        });
    }
}
