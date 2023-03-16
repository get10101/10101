use crate::calculations::calculate_liquidation_price;
use crate::db;
use crate::event;
use crate::event::EventInternal;
use crate::ln_dlc;
use crate::trade::order;
use crate::trade::order::Order;
use crate::trade::position::Position;
use crate::trade::position::PositionState;
use anyhow::Context;
use anyhow::Result;
use coordinator_commons::TradeParams;
use orderbook_commons::FilledWith;
use rust_decimal::prelude::ToPrimitive;
use state::Storage;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;
use trade::ContractSymbol;
use trade::Direction;

/// Sets up a trade with the counterparty
///
/// In a success scenario this results in creating, updating or deleting a position.
/// The DLC that represents the position will be stored in the database.
/// Errors are handled within the scope of this function.
pub async fn trade(filled: FilledWith) -> Result<()> {
    let order = db::get_order(filled.order_id).context("Could not load order from db")?;

    let trade_params = TradeParams {
        pubkey: ln_dlc::get_node_info()?.pubkey,
        contract_symbol: ContractSymbol::BtcUsd,
        leverage: order.leverage,
        quantity: order.quantity,
        direction: Direction::Long,
        filled_with: filled,
    };

    let execution_price = trade_params
        .weighted_execution_price()
        .to_f64()
        .expect("to fit into f64");
    order::handler::order_filling(order.id, execution_price)
        .context("Could not update order to filling")?;

    if let Err((reason, e)) = ln_dlc::trade(trade_params).await {
        order::handler::order_failed(Some(order.id), reason, e)
            .context("Could not set order to failed")?;
    }

    Ok(())
}

/// Fetch the positions from the database
pub async fn get_positions() -> Result<Vec<Position>> {
    // TODO: Fetch position from database

    let dummy_position = Position {
        leverage: 2.0,
        quantity: 10000.0,
        contract_symbol: ContractSymbol::BtcUsd,
        direction: Direction::Long,
        average_entry_price: 20000.0,
        liquidation_price: 14000.0,
        position_state: PositionState::Open,
        collateral: 2000,
    };

    Ok(vec![dummy_position])
}

// TODO: Remove this temporary in-memory storage of the position and safe it in the database
static POSITION: Storage<Arc<Mutex<Option<Position>>>> = Storage::new();

pub(crate) fn get() -> MutexGuard<'static, Option<Position>> {
    POSITION
        .get()
        .lock()
        .expect("failed to get lock on event hub")
}

/// Update position once an order was filled
///
/// This crates or updates the position.
/// If the position was closed we set it to `Closed` state.
pub fn position_update(filled_order: Order, collateral: u64) -> Result<()> {
    // TODO: Persist the position
    // TODO: Decide if we have to create or update the position:
    //  Probably best to have a "insert or update" function for the db that returns the position

    // TODO: Edge case: Closing a position: we will have to decide if we remove from the `positions`
    // table or just update the state of the position by introducing a `Closed` state.

    // TODO: Maybe we should change the model to *ensure* that the execution price is present past a
    // certain point; i.e. a `FilledOrder` struct?

    // store position in memory for now so we can show it to the user
    match get().into() {
        Some(_) => {
            // If it was some we set it to None
            POSITION.set(Arc::new(Mutex::new(None)));

            event::publish(&EventInternal::PositionCloseNotification(
                ContractSymbol::BtcUsd,
            ));
        }
        None => {
            let average_entry_price = filled_order.execution_price().unwrap_or(0.0);
            let have_a_position = Position {
                leverage: filled_order.leverage,
                quantity: filled_order.quantity,
                contract_symbol: filled_order.contract_symbol,
                direction: filled_order.direction,
                average_entry_price,
                // TODO: Is it correct to use the average entry price to calculate the liquidation
                // price? -> What would that mean in the UI if we already have a
                // position and trade?
                liquidation_price: calculate_liquidation_price(
                    average_entry_price,
                    filled_order.leverage,
                    filled_order.direction,
                ),
                // TODO: Remove the PnL, that has to be calculated in the UI
                position_state: PositionState::Open,
                collateral,
            };

            POSITION.set(Arc::new(Mutex::new(Some(have_a_position.clone()))));

            event::publish(&EventInternal::PositionUpdateNotification(have_a_position));
        }
    }

    Ok(())
}
