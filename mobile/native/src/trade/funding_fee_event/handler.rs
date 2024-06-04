use crate::db;
use crate::event;
use crate::event::EventInternal;
use crate::trade::FundingFeeEvent;
use anyhow::Result;
use time::OffsetDateTime;
use xxi_node::commons::ContractSymbol;

pub fn get_funding_fee_events() -> Result<Vec<FundingFeeEvent>> {
    db::get_all_funding_fee_events()
}

/// Attempt to insert a list of unpaid funding fee events. Unpaid funding fee events that are
/// already in the database are ignored.
///
/// Unpaid funding fee events that are confirmed to be new are propagated via an [`EventInternal`]
/// and returned.
pub fn handle_unpaid_funding_fee_events(
    funding_fee_events: &[FundingFeeEvent],
) -> Result<Vec<FundingFeeEvent>> {
    let new_events = db::insert_unpaid_funding_fee_events(funding_fee_events)?;

    for event in new_events.iter() {
        event::publish(&EventInternal::FundingFeeEvent(*event));
    }

    Ok(new_events)
}

pub fn mark_funding_fee_events_as_paid(
    contract_symbol: ContractSymbol,
    since: OffsetDateTime,
) -> Result<()> {
    db::mark_funding_fee_events_as_paid(contract_symbol, since)
}
