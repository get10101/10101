use crate::db;
use crate::event;
use crate::event::EventInternal;
use crate::trade::Trade;
use anyhow::Result;

pub fn new_trade(trade: Trade) -> Result<()> {
    db::insert_trade(trade)?;

    event::publish(&EventInternal::NewTrade(trade));

    Ok(())
}

pub fn get_trades() -> Result<Vec<Trade>> {
    db::get_all_trades()
}
