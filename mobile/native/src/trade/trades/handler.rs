use crate::db;
use crate::event;
use crate::event::EventInternal;
use crate::trade::Trade;
use anyhow::Result;

pub fn new_trades(trades: Vec<Trade>) -> Result<()> {
    db::insert_trades(&trades)?;

    for trade in trades {
        event::publish(&EventInternal::NewTrade(trade));
    }

    Ok(())
}

pub fn get_trades() -> Result<Vec<Trade>> {
    db::get_all_trades()
}
