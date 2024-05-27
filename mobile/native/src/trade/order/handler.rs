use crate::config;
use crate::db;
use crate::db::get_order_in_filling;
use crate::db::maybe_get_open_orders;
use crate::dlc;
use crate::dlc::check_if_signed_channel_is_confirmed;
use crate::event;
use crate::event::BackgroundTask;
use crate::event::EventInternal;
use crate::event::TaskStatus;
use crate::report_error_to_coordinator;
use crate::trade::order::orderbook_client::OrderbookClient;
use crate::trade::order::FailureReason;
use crate::trade::order::Order;
use crate::trade::order::OrderReason;
use crate::trade::order::OrderState;
use crate::trade::order::OrderType;
use crate::trade::position;
use crate::trade::position::handler::update_position_after_order_submitted;
use crate::trade::position::PositionState;
use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use bitcoin::Amount;
use dlc_manager::channel::signed_channel::SignedChannel;
use dlc_manager::channel::signed_channel::SignedChannelState;
use reqwest::Url;
use rust_decimal::prelude::ToPrimitive;
use time::Duration;
use time::OffsetDateTime;
use uuid::Uuid;
use xxi_node::commons;
use xxi_node::commons::ChannelOpeningParams;
use xxi_node::commons::Direction;
use xxi_node::node::signed_channel_state_name;

const ORDER_OUTDATED_AFTER: Duration = Duration::minutes(5);

#[derive(thiserror::Error, Debug)]
pub enum SubmitOrderError {
    /// Generic problem related to the storage layer (sqlite, sled).
    #[error("Storage failed: {0}")]
    Storage(anyhow::Error),
    #[error("DLC channel not yet confirmed")]
    UnconfirmedChannel,
    #[error("DLC Channel in invalid state: expected {expected_channel_state}, got {actual_channel_state}")]
    InvalidChannelState {
        expected_channel_state: String,
        actual_channel_state: String,
    },
    #[error("Missing DLC channel: {0}")]
    MissingChannel(String),
    #[error(
        "Another order is already being filled: {contracts} contracts {direction} at {leverage}x leverage"
    )]
    OtherOrderInFilling {
        contracts: f32,
        direction: Direction,
        leverage: f32,
    },
    #[error("Failed to post order to orderbook: {0}")]
    Orderbook(anyhow::Error),
}

pub async fn submit_order(
    order: Order,
    channel_opening_params: Option<ChannelOpeningParams>,
) -> Result<Uuid, SubmitOrderError> {
    event::publish(&EventInternal::BackgroundNotification(
        BackgroundTask::AsyncTrade(TaskStatus::Pending),
    ));

    submit_order_internal(order, channel_opening_params)
        .await
        .inspect_err(report_error_to_coordinator)
        .inspect_err(|e| {
            event::publish(&EventInternal::BackgroundNotification(
                BackgroundTask::AsyncTrade(TaskStatus::Failed(format!("{e:#}"))),
            ))
        })
}

pub async fn submit_order_internal(
    order: Order,
    channel_opening_params: Option<ChannelOpeningParams>,
) -> Result<Uuid, SubmitOrderError> {
    check_channel_state().await?;

    // Having an order in `Filling` should mean that the subchannel is in the midst of an update.
    // Since we currently only support one subchannel per app, it does not make sense to start
    // another update (by submitting a new order to the orderbook) until the current one is
    // finished.
    if let Some(filling_order) = get_order_in_filling().map_err(SubmitOrderError::Storage)? {
        return Err(SubmitOrderError::OtherOrderInFilling {
            contracts: filling_order.quantity,
            direction: filling_order.direction,
            leverage: filling_order.leverage,
        });
    }

    db::insert_order(order.clone()).map_err(SubmitOrderError::Storage)?;

    let url = format!("http://{}", config::get_http_endpoint());
    let url = Url::parse(&url).expect("correct URL");
    let orderbook_client = OrderbookClient::new(url);

    set_order_to_open_and_update_ui(order.id).map_err(SubmitOrderError::Storage)?;
    if let Err(err) = orderbook_client
        .post_new_market_order(order.clone().into(), channel_opening_params)
        .await
    {
        let order_id = order.id.clone().to_string();

        tracing::error!(order_id, "Failed to post new order: {err:#}");

        set_order_to_failed_and_update_ui(
            order.id,
            FailureReason::OrderRejected(err.to_string()),
            order.execution_price(),
        )
        .map_err(SubmitOrderError::Storage)?;

        position::handler::set_position_state(PositionState::Open)
            .context("Could not reset position to open")
            .map_err(SubmitOrderError::Storage)?;

        return Err(SubmitOrderError::Orderbook(err));
    }

    update_position_after_order_submitted(&order).map_err(SubmitOrderError::Storage)?;

    Ok(order.id)
}

/// Checks if the channel is in a valid state to post the order.
///
/// Will fail in the following scenarios
/// 1. Open position, but no channel in state [`SignedChannelState::Established`]
/// 2. Open position and not enough confirmations on the funding txid.
/// 3. No position and a channel which is not in state [`SignedChannelState::Settled`]
async fn check_channel_state() -> Result<(), SubmitOrderError> {
    let channel = dlc::get_signed_dlc_channel().map_err(SubmitOrderError::Storage)?;

    if position::handler::get_positions()
        .map_err(SubmitOrderError::Storage)?
        .first()
        .is_some()
    {
        match channel {
            Some(SignedChannel {
                state: SignedChannelState::Established { .. },
                ..
            }) => {} // all good we can continue with the order
            Some(channel) => {
                return Err(SubmitOrderError::InvalidChannelState {
                    expected_channel_state: "Established".to_string(),
                    actual_channel_state: signed_channel_state_name(&channel),
                })
            }
            None => {
                return Err(SubmitOrderError::MissingChannel(
                    "Expected established dlc channel.".to_string(),
                ))
            }
        }

        // If we have an open position, we should not allow any further trading until the current
        // DLC channel is confirmed on-chain. Otherwise we can run into pesky DLC protocol
        // failures.
        if !check_if_signed_channel_is_confirmed()
            .await
            .map_err(SubmitOrderError::Storage)?
        {
            return Err(SubmitOrderError::UnconfirmedChannel);
        }
    } else {
        match channel {
            None
            | Some(SignedChannel {
                state: SignedChannelState::Settled { .. },
                ..
            }) => {} // all good we can continue with the order
            Some(channel) => {
                return Err(SubmitOrderError::InvalidChannelState {
                    expected_channel_state: "Settled".to_string(),
                    actual_channel_state: signed_channel_state_name(&channel),
                });
            }
        }
    }

    Ok(())
}

pub(crate) fn async_order_filling(
    order: &commons::Order,
    filled_with: &commons::FilledWith,
) -> Result<()> {
    let order_type = match order.order_type {
        commons::OrderType::Market => OrderType::Market,
        commons::OrderType::Limit => OrderType::Limit {
            price: order.price.to_f32().expect("to fit into f32"),
        },
    };

    let execution_price = filled_with
        .average_execution_price()
        .to_f32()
        .expect("to fit into f32");

    let matching_fee = filled_with.order_matching_fee();

    let order = match db::get_order(order.id)? {
        None => {
            let order = Order {
                id: order.id,
                leverage: order.leverage,
                quantity: order.quantity.to_f32().expect("to fit into f32"),
                contract_symbol: order.contract_symbol,
                direction: order.direction,
                order_type,
                state: OrderState::Filling {
                    execution_price,
                    matching_fee,
                },
                creation_timestamp: order.timestamp,
                order_expiry_timestamp: order.expiry,
                reason: order.order_reason.into(),
                stable: order.stable,
                failure_reason: None,
            };

            db::insert_order(order.clone())?
        }
        Some(mut order) => {
            // the order has already been inserted to the database. Most likely because the async
            // match has already been received. We still want to retry this order as the previous
            // attempt seems to have failed.
            let order_state = OrderState::Filling {
                execution_price,
                matching_fee,
            };
            db::set_order_state_to_filling(order.id, execution_price, matching_fee)?;
            order.state = order_state;
            order
        }
    };

    event::publish(&EventInternal::OrderUpdateNotification(order.clone()));

    Ok(())
}

/// Update order to state [`OrderState::Filling`].
pub(crate) fn order_filling(
    order_id: Uuid,
    execution_price: f32,
    matching_fee: Amount,
) -> Result<()> {
    if let Err(e) = set_order_to_filling_and_update_ui(order_id, execution_price, matching_fee) {
        let e_string = format!("{e:#}");
        match order_failed(Some(order_id), FailureReason::FailedToSetToFilling, e) {
            Ok(()) => {
                tracing::debug!(
                    %order_id,
                    "Set order to failed, after failing to set it to filling"
                );
            }
            Err(e) => {
                tracing::error!(
                    %order_id,
                    "Failed to set order to failed, after failing to set it to filling: {e:#}"
                );
            }
        };

        bail!("Failed to set order {order_id} to filling: {e_string}");
    }

    Ok(())
}

/// Sets filling order to filled. Returns an error if no order in `Filling`
pub(crate) fn order_filled(order_id: Option<Uuid>) -> Result<Order> {
    let order = match order_id {
        None => get_order_in_filling(),
        Some(order_id) => db::get_order(order_id),
    }?
    .with_context(|| format!("Could not find order. order_id = {order_id:?}"))?;

    let execution_price = order.execution_price();
    let matching_fee = order.matching_fee();

    if let (Some(execution_price), Some(matching_fee)) = (execution_price, matching_fee) {
        let filled_order =
            set_order_to_filled_and_update_ui(order.id, execution_price, matching_fee)?;

        tracing::debug!(order = ?filled_order, "Order filled");

        return Ok(filled_order);
    }

    tracing::warn!(
        "Couldn't set order to filling due to missing execution price and / or matching fee"
    );
    Ok(order.clone())
}

/// Update the [`Order`]'s state to [`OrderState::Failed`].
pub(crate) fn order_failed(
    order_id: Option<Uuid>,
    reason: FailureReason,
    error: anyhow::Error,
) -> Result<()> {
    tracing::error!(?order_id, ?reason, "Failed to execute trade: {error:#}");

    let order = match order_id {
        None => get_order_in_filling(),
        Some(order_id) => db::get_order(order_id),
    }
    .inspect_err(|e| {
        // it doesn't matter that we send here an async trade failed even though we do not exactly
        // know what kind of background task failed, because the error screen looks always the same.
        event::publish(&EventInternal::BackgroundNotification(
            BackgroundTask::AsyncTrade(TaskStatus::Failed(format!("{e:#}"))),
        ))
    })?;

    let task_status = TaskStatus::Failed(format!("{error:#}"));
    let task = match order {
        Some(order) => match order.reason {
            OrderReason::Manual => BackgroundTask::AsyncTrade(task_status),
            OrderReason::Expired => BackgroundTask::Expire(task_status),
            OrderReason::CoordinatorLiquidated | OrderReason::TraderLiquidated => {
                BackgroundTask::Expire(task_status)
            }
        },
        None => {
            // if we can't find a filling order it must have been a rollover. Note this is not very
            // nice, but we are missing the required context information at the moment.
            BackgroundTask::Rollover(task_status)
        }
    };

    event::publish(&EventInternal::BackgroundNotification(task));

    if let Some(order_id) = order_id {
        set_order_to_failed_and_update_ui(order_id, reason, None)?;
    }

    // TODO: fixme. this so ugly, even a Sphynx cat is beautiful against this.
    // In this function we set the order to failed but here we try to set the position to open.
    // This is basically a roll back of a former action. It only works because we do not have a
    // concept of a closed position on the client side. However, this function is being called
    // in various places where (most of the time) we only want to set the order to failed. If we
    // were to introduce a `PostionState::Closed` the below code would be wrong and would
    // accidentally set a closed position to open again. This should be cleaned up.
    if let Err(e) = position::handler::set_position_state(PositionState::Open) {
        bail!("Could not reset position to open because of {e:#}");
    }

    Ok(())
}

pub async fn get_orders_for_ui() -> Result<Vec<Order>> {
    db::get_orders_for_ui()
}

pub fn check_open_orders() -> Result<()> {
    let open_orders = match maybe_get_open_orders() {
        Ok(orders_being_filled) => orders_being_filled,
        Err(e) => {
            bail!("Error when loading open orders from database: {e:#}");
        }
    };

    let now = OffsetDateTime::now_utc();

    for open_order in open_orders {
        tracing::debug!(?open_order, "Checking order if it is still up to date");
        if open_order.creation_timestamp + ORDER_OUTDATED_AFTER < now {
            order_failed(
                Some(open_order.id),
                FailureReason::TimedOut,
                anyhow!("Order was not matched within {ORDER_OUTDATED_AFTER:?}"),
            )?;
        }
    }

    Ok(())
}

fn set_order_to_failed_and_update_ui(
    order_id: Uuid,
    failure_reason: FailureReason,
    execution_price: Option<f32>,
) -> Result<Order> {
    let order = db::set_order_state_to_failed(order_id, failure_reason.into(), execution_price)
        .with_context(|| format!("Failed to update order {order_id} to state failed"))?;

    ui_update(order.clone());

    Ok(order)
}

fn set_order_to_open_and_update_ui(order_id: Uuid) -> Result<Order> {
    let order = db::set_order_state_to_open(order_id)
        .with_context(|| format!("Failed to update order {order_id} to state failed"))?;

    ui_update(order.clone());

    Ok(order)
}

fn set_order_to_filled_and_update_ui(
    order_id: Uuid,
    execution_price: f32,
    matching_fee: Amount,
) -> Result<Order> {
    let order = db::set_order_state_to_filled(order_id, execution_price, matching_fee)
        .with_context(|| format!("Failed to update order {order_id} to state filled"))?;

    ui_update(order.clone());

    Ok(order)
}

fn set_order_to_filling_and_update_ui(
    order_id: Uuid,
    execution_price: f32,
    matching_fee: Amount,
) -> Result<Order> {
    let order = db::set_order_state_to_filling(order_id, execution_price, matching_fee)
        .with_context(|| format!("Failed to update order {order_id} to state filling"))?;

    ui_update(order.clone());

    Ok(order)
}

fn ui_update(order: Order) {
    event::publish(&EventInternal::OrderUpdateNotification(order));
}
