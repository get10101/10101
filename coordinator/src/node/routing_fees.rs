use crate::db;
use crate::node::Node;
use crate::routing_fee::models::NewRoutingFee;
use anyhow::anyhow;
use anyhow::Context;
use lightning::util::events::Event;

/// Save the routing fee in the database upon `PaymentForwarded` event
///
/// Only takes regular routing fees into account. This function does not handle force-close
/// scenarios where the `fee_earned_msat` is set to `None`.
pub async fn handle(node: Node, event: Option<Event>) {
    //
    if let Some(Event::PaymentForwarded {
        fee_earned_msat: Some(fee_earned_msat),
        prev_channel_id,
        next_channel_id,
        ..
    }) = event
    {
        if let Err(e) = tokio::task::spawn_blocking(move || {
            let mut conn = node
                .pool
                .get()
                .context("Failed to acquire database connection")?;

            db::routing_fees::insert(
                NewRoutingFee {
                    amount_msats: fee_earned_msat,
                    prev_channel_id,
                    next_channel_id,
                },
                &mut conn,
            )
            .context("Failed to insert routing fee into database")?;

            anyhow::Ok(())
        })
        .await
        .map_err(|e| anyhow!(e))
        {
            tracing::error!(?event, "Failed to insert routing fee: {e:#}")
        }
    }
}
