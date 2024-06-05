//! The `protocol_funding_fee_events` table defines the relationship between funding fee events and
//! the DLC protocol that will resolve them.

use crate::schema::protocol_funding_fee_events;
use diesel::prelude::*;
use xxi_node::node::ProtocolId;

pub fn insert_protocol_funding_fee_event(
    conn: &mut PgConnection,
    protocol_id: ProtocolId,
    funding_fee_event_ids: &[i32],
) -> QueryResult<()> {
    if funding_fee_event_ids.is_empty() {
        tracing::debug!(
            %protocol_id,
            "Protocol without outstanding funding fee events"
        );

        return Ok(());
    }

    let values = funding_fee_event_ids
        .iter()
        .map(|funding_fee_event_id| {
            (
                protocol_funding_fee_events::protocol_id.eq(protocol_id.to_uuid()),
                protocol_funding_fee_events::funding_fee_event_id.eq(*funding_fee_event_id),
            )
        })
        .collect::<Vec<_>>();

    let affected_rows = diesel::insert_into(protocol_funding_fee_events::table)
        .values(values)
        .execute(conn)?;

    if affected_rows == 0 {
        return Err(diesel::result::Error::NotFound);
    }

    Ok(())
}
