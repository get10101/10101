use crate::config;
use crate::db;
use crate::event;
use crate::event::EventInternal;
use crate::trade::order::FailureReason;
use crate::trade::order::Order;
use crate::trade::order::OrderState;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use uuid::Uuid;

pub async fn submit_order(order: Order) -> Result<()> {
    db::insert_order(order)?;

    // todo: this model should include the contract_symbol
    let new_order = trade::NewOrder {
        price: dec!(22_000), // todo: replace with actual btcusd rate price
        quantity: Decimal::try_from(order.quantity)?,
        maker_id: "".to_string(), // todo: what should be provided on the maker_id?
        direction: order.direction,
    };

    let client = reqwest::Client::new();
    client
        .post(format!(
            "http://{}/api/orderbook/orders",
            config::get_http_endpoint()
        ))
        .json(&new_order)
        .send()
        .await
        .context("Failed submit order to order book")?;

    ui_update(order);

    Ok(())
}

pub(crate) fn order_filling(order_id: Uuid, execution_price: f64) -> Result<()> {
    let filling_state = OrderState::Filling { execution_price };

    if let Err(e) = db::update_order_state(order_id, filling_state) {
        tracing::error!("Failed to update state of {order_id} to {filling_state:?}: {e:#}");
        order_failed(Some(order_id), FailureReason::FailedToSetToFilling, e)?;

        bail!("Failed to update state of {order_id} to {filling_state:?}")
    }

    Ok(())
}

pub(crate) fn order_filled() -> Result<Order> {
    let order_being_filled = get_order_being_filled()?;

    // Default the execution price in case we don't know
    let execution_price = order_being_filled.execution_price().unwrap_or(0.0);

    let filled_order = update_order_state(
        order_being_filled.id,
        OrderState::Filled { execution_price },
    )?;
    Ok(filled_order)
}

/// Update order state to failed
///
/// If the order_id is know we load the order by id and set it to failed.
/// If the order_id is not known we load the order that is currently in `Filling` state and set it
/// to failed.
pub(crate) fn order_failed(
    order_id: Option<Uuid>,
    reason: FailureReason,
    error: anyhow::Error,
) -> Result<()> {
    tracing::error!("Failed to execute trade for order {order_id:?}: {reason:?}: {error:#}");

    let order_id = match order_id {
        None => get_order_being_filled()?.id,
        Some(order_id) => order_id,
    };

    update_order_state(order_id, OrderState::Failed { reason })?;

    Ok(())
}

pub async fn get_orders_for_ui() -> Result<Vec<Order>> {
    db::get_orders_for_ui()
}

fn get_order_being_filled() -> Result<Order> {
    let order_being_filled = match db::maybe_get_order_in_filling() {
        Ok(Some(order_being_filled)) => order_being_filled,
        Ok(None) => {
            bail!("There is no order in state filled in the database");
        }
        Err(e) => {
            bail!("Error when loading order being filled from database: {e:#}");
        }
    };

    Ok(order_being_filled)
}

fn update_order_state(order_id: Uuid, state: OrderState) -> Result<Order> {
    db::update_order_state(order_id, state)
        .with_context(|| format!("Failed to update order {order_id} with state {state:?}"))?;

    let order = db::get_order(order_id).with_context(|| {
        format!("Failed to load order {order_id} after updating it to state {state:?}")
    })?;

    ui_update(order);

    Ok(order)
}

fn ui_update(order: Order) {
    event::publish(&EventInternal::OrderUpdateNotification(order));
}
