use crate::schema;
use crate::schema::channels;
use crate::schema::orders;
use crate::schema::positions;
use crate::schema::spendable_outputs;
use crate::schema::trades;
use crate::schema::transactions;
use crate::trade::order::InvalidSubchannelOffer;
use anyhow::anyhow;
use anyhow::bail;
use anyhow::ensure;
use anyhow::Result;
use bitcoin::Amount;
use bitcoin::SignedAmount;
use bitcoin::Txid;
use diesel;
use diesel::prelude::*;
use diesel::sql_types::Text;
use diesel::AsExpression;
use diesel::FromSqlRow;
use diesel::Queryable;
use lightning::util::ser::Readable;
use lightning::util::ser::Writeable;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::Deserialize;
use serde::Serialize;
use std::str::FromStr;
use time::OffsetDateTime;
use trade;
use uuid::Uuid;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Invalid id when converting string to uuid: {0}")]
    InvalidId(#[from] uuid::Error),
    #[error("Limit order has to have a price")]
    MissingPriceForLimitOrder,
    #[error("A filling or filled order has to have an execution price")]
    MissingExecutionPrice,
    #[error("A filled order has to have a matching fee")]
    MissingMatchingFee,
}

#[derive(Queryable, QueryableByName, Insertable, Debug, Clone, PartialEq)]
#[diesel(table_name = orders)]
pub(crate) struct Order {
    pub id: String,
    pub leverage: f32,
    pub quantity: f32,
    pub contract_symbol: ContractSymbol,
    pub direction: Direction,
    pub order_type: OrderType,
    pub state: OrderState,
    pub creation_timestamp: i64,
    pub limit_price: Option<f32>,
    pub execution_price: Option<f32>,
    pub failure_reason: Option<FailureReason>,
    pub order_expiry_timestamp: i64,
    pub reason: OrderReason,
    pub stable: bool,
    pub matching_fee_sats: Option<i64>,
}

impl Order {
    /// inserts the given order into the db. Returns the order if successful
    pub fn insert(order: Order, conn: &mut SqliteConnection) -> Result<Order> {
        let affected_rows = diesel::insert_into(orders::table)
            .values(&order)
            .execute(conn)?;

        if affected_rows > 0 {
            Ok(order)
        } else {
            bail!("Could not insert order")
        }
    }

    /// Sets all filling orders to failed. Only be used for emergency recoveries!
    pub fn set_all_filling_orders_to_failed(conn: &mut SqliteConnection) -> Result<()> {
        let affected_rows = diesel::update(orders::table)
            .filter(schema::orders::state.eq(OrderState::Filling))
            .set((
                orders::state.eq(OrderState::Failed),
                orders::failure_reason.eq(FailureReason::Unknown),
            ))
            .execute(conn)?;

        tracing::info!("Updated {affected_rows} orders from Filling to Failed");

        Ok(())
    }

    pub fn set_order_state_to_failed(
        order_id: String,
        execution_price: Option<f32>,
        matching_fee: Option<Amount>,
        failure_reason: FailureReason,
        conn: &mut SqliteConnection,
    ) -> Result<Order> {
        Self::update_state(
            order_id,
            OrderState::Failed,
            execution_price,
            matching_fee,
            Some(failure_reason),
            conn,
        )
    }

    pub fn set_order_state_to_open(order_id: String, conn: &mut SqliteConnection) -> Result<Order> {
        Self::update_state(order_id, OrderState::Open, None, None, None, conn)
    }

    pub fn set_order_state_to_filling(
        order_id: Uuid,
        execution_price: f32,
        matching_fee: Amount,
        conn: &mut SqliteConnection,
    ) -> Result<Order> {
        Self::update_state(
            order_id.to_string(),
            OrderState::Filling,
            Some(execution_price),
            Some(matching_fee),
            None,
            conn,
        )
    }

    pub fn set_order_state_to_filled(
        order_id: Uuid,
        execution_price: f32,
        matching_fee: Amount,
        conn: &mut SqliteConnection,
    ) -> Result<Order> {
        Self::update_state(
            order_id.to_string(),
            OrderState::Filled,
            Some(execution_price),
            Some(matching_fee),
            None,
            conn,
        )
    }

    /// updates the status of the given order in the db
    fn update_state(
        order_id: String,
        order_state: OrderState,
        execution_price: Option<f32>,
        matching_fee: Option<Amount>,
        failure_reason: Option<FailureReason>,
        conn: &mut SqliteConnection,
    ) -> Result<Order> {
        conn.exclusive_transaction::<Order, _, _>(|conn| {
            let order: Order = orders::table
                .filter(schema::orders::id.eq(order_id.clone()))
                .first(conn)?;

            let current_state = order.state;
            match current_state.next_state(order_state) {
                Some(next_state) => {
                    let affected_rows = diesel::update(orders::table)
                        .filter(schema::orders::id.eq(order_id.clone()))
                        .set(schema::orders::state.eq(next_state))
                        .execute(conn)?;

                    if affected_rows == 0 {
                        bail!("Could not update order state")
                    }

                    tracing::info!(new_state = ?next_state, %order_id, "Updated order state");
                }
                None => {
                    tracing::debug!(?current_state, ?order_state, "Ignoring latest state update");
                }
            }

            if let Some(execution_price) = execution_price {
                let affected_rows = diesel::update(orders::table)
                    .filter(schema::orders::id.eq(order_id.clone()))
                    .set(schema::orders::execution_price.eq(execution_price))
                    .execute(conn)?;

                if affected_rows == 0 {
                    bail!("Could not update order execution price")
                }
            }

            if let Some(matching_fee) = matching_fee {
                let affected_rows = diesel::update(orders::table)
                    .filter(schema::orders::id.eq(order_id.clone()))
                    .set(schema::orders::matching_fee_sats.eq(matching_fee.to_sat() as i64))
                    .execute(conn)?;

                if affected_rows == 0 {
                    bail!("Could not update order matching fee")
                }
            }

            if let Some(failure_reason) = failure_reason {
                let affected_rows = diesel::update(orders::table)
                    .filter(schema::orders::id.eq(order_id.clone()))
                    .set(schema::orders::failure_reason.eq(failure_reason))
                    .execute(conn)?;

                if affected_rows == 0 {
                    bail!("Could not update order failure reason")
                }
            }

            let order = orders::table
                .filter(schema::orders::id.eq(order_id.clone()))
                .first(conn)?;

            Ok(order)
        })
    }

    pub fn get(order_id: String, conn: &mut SqliteConnection) -> QueryResult<Option<Order>> {
        orders::table
            .filter(schema::orders::id.eq(order_id))
            .first(conn)
            .optional()
    }

    /// Fetch all orders that are not in initial and rejected state
    pub fn get_without_rejected_and_initial(
        conn: &mut SqliteConnection,
    ) -> QueryResult<Vec<Order>> {
        orders::table
            .filter(
                schema::orders::state
                    .ne(OrderState::Initial)
                    .and(schema::orders::state.ne(OrderState::Rejected)),
            )
            .load(conn)
    }

    /// Gets any async order in the database. An async order is defined by any order which has been
    /// generated by the orderbook. e.g. if the position expired.
    pub fn get_async_order(conn: &mut SqliteConnection) -> QueryResult<Option<Order>> {
        orders::table
            .filter(
                orders::state
                    .eq(OrderState::Filling)
                    .and(orders::reason.eq(OrderReason::Expired)),
            )
            .first(conn)
            .optional()
    }

    pub fn get_by_state(
        order_state: OrderState,
        conn: &mut SqliteConnection,
    ) -> QueryResult<Vec<Order>> {
        orders::table
            .filter(schema::orders::state.eq(order_state))
            .load(conn)
    }

    /// Deletes given order from DB, in case of success, returns > 0, else 0 or Err
    pub fn delete(order_id: String, conn: &mut SqliteConnection) -> QueryResult<usize> {
        diesel::delete(orders::table)
            .filter(orders::id.eq(order_id))
            .execute(conn)
    }
}

impl From<crate::trade::order::Order> for Order {
    fn from(value: crate::trade::order::Order) -> Self {
        let (order_type, limit_price) = value.order_type.into();
        let execution_price = value.execution_price();
        let matching_fee = value.matching_fee();

        let (status, _, failure_reason) = value.state.into();

        Order {
            id: value.id.to_string(),
            leverage: value.leverage,
            quantity: value.quantity,
            contract_symbol: value.contract_symbol.into(),
            direction: value.direction.into(),
            order_type,
            state: status,
            creation_timestamp: value.creation_timestamp.unix_timestamp(),
            limit_price,
            execution_price,
            failure_reason,
            order_expiry_timestamp: value.order_expiry_timestamp.unix_timestamp(),
            reason: value.reason.into(),
            stable: value.stable,
            matching_fee_sats: matching_fee.map(|fee| fee.to_sat() as i64),
        }
    }
}

impl From<crate::trade::order::OrderReason> for OrderReason {
    fn from(value: crate::trade::order::OrderReason) -> Self {
        match value {
            crate::trade::order::OrderReason::Manual => OrderReason::Manual,
            crate::trade::order::OrderReason::Expired => OrderReason::Expired,
            crate::trade::order::OrderReason::CoordinatorLiquidated => {
                OrderReason::CoordinatorLiquidated
            }
            crate::trade::order::OrderReason::TraderLiquidated => OrderReason::TraderLiquidated,
        }
    }
}

impl From<OrderReason> for crate::trade::order::OrderReason {
    fn from(value: OrderReason) -> Self {
        match value {
            OrderReason::Manual => crate::trade::order::OrderReason::Manual,
            OrderReason::Expired => crate::trade::order::OrderReason::Expired,
            OrderReason::CoordinatorLiquidated => {
                crate::trade::order::OrderReason::CoordinatorLiquidated
            }
            OrderReason::TraderLiquidated => crate::trade::order::OrderReason::TraderLiquidated,
        }
    }
}

impl TryFrom<Order> for crate::trade::order::Order {
    type Error = Error;

    fn try_from(value: Order) -> std::result::Result<Self, Self::Error> {
        let order = crate::trade::order::Order {
            id: Uuid::parse_str(value.id.as_str()).map_err(Error::InvalidId)?,
            leverage: value.leverage,
            quantity: value.quantity,
            contract_symbol: value.contract_symbol.into(),
            direction: value.direction.into(),
            order_type: (value.order_type, value.limit_price).try_into()?,
            state: derive_order_state(
                value.state,
                value.execution_price,
                value.matching_fee_sats,
                value.failure_reason.clone(),
            )?,
            creation_timestamp: OffsetDateTime::from_unix_timestamp(value.creation_timestamp)
                .expect("unix timestamp to fit in itself"),
            order_expiry_timestamp: OffsetDateTime::from_unix_timestamp(
                value.order_expiry_timestamp,
            )
            .expect("unix timestamp to fit in itself"),
            reason: value.reason.into(),
            stable: value.stable,
            failure_reason: value.failure_reason.map(|reason| reason.into()),
        };

        Ok(order)
    }
}

fn derive_order_state(
    order_state: OrderState,
    execution_price: Option<f32>,
    matching_fee: Option<i64>,
    failure_reason: Option<FailureReason>,
) -> Result<crate::trade::order::OrderState, Error> {
    let state = match order_state {
        OrderState::Initial => crate::trade::order::OrderState::Initial,
        OrderState::Rejected => crate::trade::order::OrderState::Rejected,
        OrderState::Open => crate::trade::order::OrderState::Open,
        OrderState::Filling => match execution_price {
            None => return Err(Error::MissingExecutionPrice),
            Some(execution_price) => crate::trade::order::OrderState::Filling {
                execution_price,
                matching_fee: Amount::from_sat(matching_fee.unwrap_or_default() as u64),
            },
        },
        OrderState::Failed => crate::trade::order::OrderState::Failed {
            execution_price,
            reason: failure_reason.unwrap_or_default().into(),
        },
        OrderState::Filled => {
            let execution_price = if let Some(execution_price) = execution_price {
                execution_price
            } else {
                return Err(Error::MissingExecutionPrice);
            };

            let matching_fee = if let Some(matching_fee) = matching_fee {
                matching_fee
            } else {
                return Err(Error::MissingMatchingFee);
            };

            crate::trade::order::OrderState::Filled {
                execution_price,
                matching_fee: Amount::from_sat(matching_fee as u64),
            }
        }
    };

    Ok(state)
}

#[derive(Queryable, QueryableByName, Insertable, Debug, Clone, PartialEq)]
#[diesel(table_name = positions)]
pub(crate) struct Position {
    pub contract_symbol: ContractSymbol,
    pub leverage: f32,
    pub quantity: f32,
    pub direction: Direction,
    pub average_entry_price: f32,
    pub liquidation_price: f32,
    pub state: PositionState,
    pub collateral: i64,
    pub creation_timestamp: i64,
    pub expiry_timestamp: i64,
    pub updated_timestamp: i64,
    pub stable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, FromSqlRow, AsExpression)]
#[diesel(sql_type = Text)]
pub enum PositionState {
    Open,
    Closing,
    Rollover,
    Resizing,
}

impl Position {
    /// inserts the given position into the db. Returns the position if successful
    pub fn insert(position: Position, conn: &mut SqliteConnection) -> Result<Position> {
        let affected_rows = diesel::insert_into(positions::table)
            .values(&position)
            .execute(conn)?;

        if affected_rows > 0 {
            Ok(position)
        } else {
            bail!("Could not insert position")
        }
    }

    pub fn get_all(conn: &mut SqliteConnection) -> QueryResult<Vec<Position>> {
        positions::table.load(conn)
    }

    /// Update the status of the [`Position`] identified by the given [`ContractSymbol`].
    pub fn update_state(
        contract_symbol: ContractSymbol,
        state: PositionState,
        conn: &mut SqliteConnection,
    ) -> Result<Position> {
        let affected_rows = diesel::update(positions::table)
            .filter(schema::positions::contract_symbol.eq(contract_symbol))
            .set(schema::positions::state.eq(state))
            .execute(conn)?;

        if affected_rows == 0 {
            bail!("Could not update position")
        }

        let position = positions::table
            .filter(positions::contract_symbol.eq(contract_symbol))
            .first(conn)?;

        Ok(position)
    }

    // sets the position to rollover and updates the new expiry timestamp.
    pub fn rollover(
        conn: &mut SqliteConnection,
        contract_symbol: ContractSymbol,
        expiry_timestamp: OffsetDateTime,
    ) -> Result<()> {
        let affected_rows = diesel::update(positions::table)
            .filter(schema::positions::contract_symbol.eq(contract_symbol))
            .set((
                positions::expiry_timestamp.eq(expiry_timestamp.unix_timestamp()),
                positions::state.eq(PositionState::Rollover),
                positions::updated_timestamp.eq(OffsetDateTime::now_utc().unix_timestamp()),
            ))
            .execute(conn)?;

        ensure!(affected_rows > 0, "Could not set position to rollover");

        Ok(())
    }

    /// Updates the status of the given order in the DB.
    pub fn update_position(conn: &mut SqliteConnection, position: Position) -> Result<()> {
        let Position {
            contract_symbol,
            leverage,
            quantity,
            direction,
            average_entry_price,
            liquidation_price,
            state,
            collateral,
            creation_timestamp: _,
            expiry_timestamp,
            updated_timestamp,
            ..
        } = position;

        let affected_rows = diesel::update(positions::table)
            .filter(schema::positions::contract_symbol.eq(contract_symbol))
            .set((
                positions::leverage.eq(leverage),
                positions::quantity.eq(quantity),
                positions::direction.eq(direction),
                positions::average_entry_price.eq(average_entry_price),
                positions::liquidation_price.eq(liquidation_price),
                positions::state.eq(state),
                positions::collateral.eq(collateral),
                positions::expiry_timestamp.eq(expiry_timestamp),
                positions::updated_timestamp.eq(updated_timestamp),
            ))
            .execute(conn)?;

        if affected_rows == 0 {
            bail!("Could not update position")
        }

        Ok(())
    }

    // TODO: This is obviously only for the MVP :)
    /// deletes all positions in the database
    pub fn delete_all(conn: &mut SqliteConnection) -> QueryResult<usize> {
        diesel::delete(positions::table).execute(conn)
    }
}

impl From<Position> for crate::trade::position::Position {
    fn from(value: Position) -> Self {
        Self {
            leverage: value.leverage,
            quantity: value.quantity,
            contract_symbol: value.contract_symbol.into(),
            direction: value.direction.into(),
            average_entry_price: value.average_entry_price,
            liquidation_price: value.liquidation_price,
            position_state: value.state.into(),
            collateral: value.collateral as u64,
            expiry: OffsetDateTime::from_unix_timestamp(value.expiry_timestamp)
                .expect("to fit into unix timestamp"),
            updated: OffsetDateTime::from_unix_timestamp(value.updated_timestamp)
                .expect("to fit into unix timestamp"),
            created: OffsetDateTime::from_unix_timestamp(value.creation_timestamp)
                .expect("to fit into unix timestamp"),
            stable: value.stable,
        }
    }
}

impl From<crate::trade::position::Position> for Position {
    fn from(value: crate::trade::position::Position) -> Self {
        Self {
            contract_symbol: value.contract_symbol.into(),
            leverage: value.leverage,
            quantity: value.quantity,
            direction: value.direction.into(),
            average_entry_price: value.average_entry_price,
            liquidation_price: value.liquidation_price,
            state: value.position_state.into(),
            collateral: value.collateral as i64,
            creation_timestamp: OffsetDateTime::now_utc().unix_timestamp(),
            updated_timestamp: OffsetDateTime::now_utc().unix_timestamp(),
            expiry_timestamp: value.expiry.unix_timestamp(),
            stable: value.stable,
        }
    }
}

impl From<crate::trade::position::PositionState> for PositionState {
    fn from(value: crate::trade::position::PositionState) -> Self {
        match value {
            crate::trade::position::PositionState::Open => PositionState::Open,
            crate::trade::position::PositionState::Closing => PositionState::Closing,
            crate::trade::position::PositionState::Rollover => PositionState::Rollover,
            crate::trade::position::PositionState::Resizing => PositionState::Resizing,
        }
    }
}

impl From<PositionState> for crate::trade::position::PositionState {
    fn from(value: PositionState) -> Self {
        match value {
            PositionState::Open => crate::trade::position::PositionState::Open,
            PositionState::Closing => crate::trade::position::PositionState::Closing,
            PositionState::Rollover => crate::trade::position::PositionState::Rollover,
            PositionState::Resizing => crate::trade::position::PositionState::Resizing,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, FromSqlRow, AsExpression)]
#[diesel(sql_type = Text)]
pub enum ContractSymbol {
    BtcUsd,
}

impl From<trade::ContractSymbol> for ContractSymbol {
    fn from(value: trade::ContractSymbol) -> Self {
        match value {
            trade::ContractSymbol::BtcUsd => ContractSymbol::BtcUsd,
        }
    }
}

impl From<ContractSymbol> for trade::ContractSymbol {
    fn from(value: ContractSymbol) -> Self {
        match value {
            ContractSymbol::BtcUsd => trade::ContractSymbol::BtcUsd,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, FromSqlRow, AsExpression)]
#[diesel(sql_type = Text)]
pub enum Direction {
    Long,
    Short,
}

impl From<trade::Direction> for Direction {
    fn from(value: trade::Direction) -> Self {
        match value {
            trade::Direction::Long => Direction::Long,
            trade::Direction::Short => Direction::Short,
        }
    }
}

impl From<Direction> for trade::Direction {
    fn from(value: Direction) -> Self {
        match value {
            Direction::Long => trade::Direction::Long,
            Direction::Short => trade::Direction::Short,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, FromSqlRow, AsExpression)]
#[diesel(sql_type = Text)]
pub enum OrderType {
    Market,
    Limit,
}

impl From<crate::trade::order::OrderType> for (OrderType, Option<f32>) {
    fn from(value: crate::trade::order::OrderType) -> Self {
        match value {
            crate::trade::order::OrderType::Market => (OrderType::Market, None),
            crate::trade::order::OrderType::Limit { price } => (OrderType::Limit, Some(price)),
        }
    }
}

impl TryFrom<(OrderType, Option<f32>)> for crate::trade::order::OrderType {
    type Error = Error;

    fn try_from(value: (OrderType, Option<f32>)) -> std::result::Result<Self, Self::Error> {
        let order_type = match value.0 {
            OrderType::Market => crate::trade::order::OrderType::Market,
            OrderType::Limit => match value.1 {
                None => return Err(Error::MissingPriceForLimitOrder),
                Some(price) => crate::trade::order::OrderType::Limit { price },
            },
        };

        Ok(order_type)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, FromSqlRow, AsExpression)]
#[diesel(sql_type = Text)]
pub enum OrderReason {
    Manual,
    Expired,
    CoordinatorLiquidated,
    TraderLiquidated,
}

#[derive(Debug, Clone, Copy, PartialEq, FromSqlRow, AsExpression)]
#[diesel(sql_type = Text)]
pub enum OrderState {
    Initial,
    Rejected,
    Open,
    Filling,
    Failed,
    Filled,
}

impl OrderState {
    /// Determines what state to go to after learning about the latest [`OrderState`] update.
    ///
    /// If the state should remain unchanged [`None`] is returned.
    ///
    /// TODO: It might be a good idea to introduce a different type that models `OrderUpdates`
    /// explicitly.
    fn next_state(&self, latest: Self) -> Option<Self> {
        match (self, latest) {
            // We can go from `Initial` to any other state
            (OrderState::Initial, latest) => Some(latest),
            // `Rejected` is a final state
            (OrderState::Rejected, _) => None,
            // We cannnot go back to `Initial` if the order is already `Open`
            (OrderState::Open, OrderState::Initial) => None,
            (OrderState::Open, latest) => Some(latest),
            // We cannot go back to `Initial` or `Open` if the order is already `Filling`
            (OrderState::Filling, OrderState::Initial | OrderState::Open) => None,
            (OrderState::Filling, latest) => Some(latest),
            // `Failed` is a final state
            (OrderState::Failed, _) => None,
            // `Filled` is a final state
            (OrderState::Filled, _) => None,
        }
    }
}

impl From<crate::trade::order::OrderState> for (OrderState, Option<f32>, Option<FailureReason>) {
    fn from(value: crate::trade::order::OrderState) -> Self {
        match value {
            crate::trade::order::OrderState::Initial => (OrderState::Initial, None, None),
            crate::trade::order::OrderState::Rejected => (OrderState::Rejected, None, None),
            crate::trade::order::OrderState::Open => (OrderState::Open, None, None),
            crate::trade::order::OrderState::Failed {
                execution_price,
                reason,
            } => (OrderState::Failed, execution_price, Some(reason.into())),
            crate::trade::order::OrderState::Filled {
                execution_price, ..
            } => (OrderState::Filled, Some(execution_price), None),
            crate::trade::order::OrderState::Filling {
                execution_price, ..
            } => (OrderState::Filling, Some(execution_price), None),
        }
    }
}

#[derive(Debug, Clone, PartialEq, FromSqlRow, AsExpression, Serialize, Deserialize, Default)]
#[diesel(sql_type = Text)]
pub enum FailureReason {
    FailedToSetToFilling,
    TradeRequest,
    TradeResponse(String),
    CollabRevert,
    OrderNotAcceptable,
    TimedOut,
    SubchannelOfferOutdated,
    SubchannelOfferDateUndetermined,
    SubchannelOfferUnacceptable,
    OrderRejected(String),
    #[default]
    Unknown,
}

impl From<FailureReason> for crate::trade::order::FailureReason {
    fn from(value: FailureReason) -> Self {
        match value {
            FailureReason::TradeRequest => crate::trade::order::FailureReason::TradeRequest,
            FailureReason::TradeResponse(details) => {
                crate::trade::order::FailureReason::TradeResponse(details)
            }
            FailureReason::CollabRevert => crate::trade::order::FailureReason::CollabRevert,
            FailureReason::FailedToSetToFilling => {
                crate::trade::order::FailureReason::FailedToSetToFilling
            }
            FailureReason::OrderNotAcceptable => {
                crate::trade::order::FailureReason::OrderNotAcceptable
            }
            FailureReason::TimedOut => crate::trade::order::FailureReason::TimedOut,
            FailureReason::SubchannelOfferOutdated => {
                crate::trade::order::FailureReason::InvalidDlcOffer(
                    InvalidSubchannelOffer::Outdated,
                )
            }
            FailureReason::SubchannelOfferDateUndetermined => {
                crate::trade::order::FailureReason::InvalidDlcOffer(
                    InvalidSubchannelOffer::UndeterminedMaturityDate,
                )
            }
            FailureReason::SubchannelOfferUnacceptable => {
                crate::trade::order::FailureReason::InvalidDlcOffer(
                    InvalidSubchannelOffer::Unacceptable,
                )
            }
            FailureReason::OrderRejected(reason) => {
                crate::trade::order::FailureReason::OrderRejected(reason)
            }
            FailureReason::Unknown => crate::trade::order::FailureReason::Unknown,
        }
    }
}

impl From<crate::trade::order::FailureReason> for FailureReason {
    fn from(value: crate::trade::order::FailureReason) -> Self {
        match value {
            crate::trade::order::FailureReason::TradeRequest => FailureReason::TradeRequest,
            crate::trade::order::FailureReason::TradeResponse(details) => {
                FailureReason::TradeResponse(details)
            }
            crate::trade::order::FailureReason::CollabRevert => FailureReason::CollabRevert,
            crate::trade::order::FailureReason::FailedToSetToFilling => {
                FailureReason::FailedToSetToFilling
            }
            crate::trade::order::FailureReason::OrderNotAcceptable => {
                FailureReason::OrderNotAcceptable
            }
            crate::trade::order::FailureReason::TimedOut => FailureReason::TimedOut,
            crate::trade::order::FailureReason::InvalidDlcOffer(reason) => match reason {
                InvalidSubchannelOffer::Outdated => FailureReason::SubchannelOfferOutdated,
                InvalidSubchannelOffer::UndeterminedMaturityDate => {
                    FailureReason::SubchannelOfferDateUndetermined
                }
                InvalidSubchannelOffer::Unacceptable => FailureReason::SubchannelOfferUnacceptable,
            },
            crate::trade::order::FailureReason::OrderRejected(reason) => {
                FailureReason::OrderRejected(reason)
            }
            crate::trade::order::FailureReason::Unknown => FailureReason::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, FromSqlRow, AsExpression)]
#[diesel(sql_type = Text)]
pub enum Flow {
    Inbound,
    Outbound,
}

impl From<ln_dlc_node::PaymentFlow> for Flow {
    fn from(value: ln_dlc_node::PaymentFlow) -> Self {
        match value {
            ln_dlc_node::PaymentFlow::Inbound => Self::Inbound,
            ln_dlc_node::PaymentFlow::Outbound => Self::Outbound,
        }
    }
}

impl From<Flow> for ln_dlc_node::PaymentFlow {
    fn from(value: Flow) -> Self {
        match value {
            Flow::Inbound => Self::Inbound,
            Flow::Outbound => Self::Outbound,
        }
    }
}

#[derive(Insertable, Debug, Clone, PartialEq)]
#[diesel(table_name = spendable_outputs)]
pub(crate) struct SpendableOutputInsertable {
    #[diesel(sql_type = Text)]
    pub outpoint: String,
    #[diesel(sql_type = Text)]
    pub descriptor: String,
}

impl SpendableOutputInsertable {
    pub fn insert(output: SpendableOutputInsertable, conn: &mut SqliteConnection) -> Result<()> {
        let affected_rows = diesel::insert_into(spendable_outputs::table)
            .values(&output)
            .execute(conn)?;

        ensure!(affected_rows > 0, "Could not insert spendable");

        Ok(())
    }
}

#[derive(Queryable, Debug, Clone, PartialEq)]
#[diesel(table_name = spendable_outputs)]
pub(crate) struct SpendableOutputQueryable {
    pub id: i32,
    pub outpoint: String,
    pub descriptor: String,
}

impl SpendableOutputQueryable {
    pub fn get(
        outpoint: lightning::chain::transaction::OutPoint,
        conn: &mut SqliteConnection,
    ) -> QueryResult<Self> {
        let outpoint = outpoint_to_string(outpoint);

        spendable_outputs::table
            .filter(schema::spendable_outputs::outpoint.eq(outpoint))
            .first(conn)
    }

    pub fn delete(
        outpoint: lightning::chain::transaction::OutPoint,
        conn: &mut SqliteConnection,
    ) -> Result<()> {
        let outpoint = outpoint_to_string(outpoint);

        let affected_rows = diesel::delete(
            spendable_outputs::table.filter(schema::spendable_outputs::outpoint.eq(outpoint)),
        )
        .execute(conn)?;

        ensure!(affected_rows > 0, "Could not delete spendable output");

        Ok(())
    }

    pub fn get_all(conn: &mut SqliteConnection) -> QueryResult<Vec<SpendableOutputQueryable>> {
        spendable_outputs::table.load(conn)
    }
}

fn outpoint_to_string(outpoint: lightning::chain::transaction::OutPoint) -> String {
    format!("{}:{}", outpoint.txid, outpoint.index)
}

impl
    From<(
        lightning::chain::transaction::OutPoint,
        lightning::sign::SpendableOutputDescriptor,
    )> for SpendableOutputInsertable
{
    fn from(
        (outpoint, descriptor): (
            lightning::chain::transaction::OutPoint,
            lightning::sign::SpendableOutputDescriptor,
        ),
    ) -> Self {
        let outpoint = outpoint_to_string(outpoint);
        let descriptor = hex::encode(descriptor.encode());

        Self {
            outpoint,
            descriptor,
        }
    }
}

impl TryFrom<SpendableOutputQueryable> for lightning::sign::SpendableOutputDescriptor {
    type Error = anyhow::Error;

    fn try_from(value: SpendableOutputQueryable) -> Result<Self, Self::Error> {
        let bytes = hex::decode(value.descriptor)?;
        let descriptor = Self::read(&mut lightning::io::Cursor::new(bytes))
            .map_err(|e| anyhow!("Failed to decode spendable output descriptor: {e}"))?;

        Ok(descriptor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, FromSqlRow, AsExpression)]
#[diesel(sql_type = Text)]
pub enum ChannelState {
    Announced,
    Pending,
    OpenUnpaid,
    Open,
    Closed,
    ForceClosedRemote,
    ForceClosedLocal,
}

#[derive(Insertable, QueryableByName, Queryable, Debug, Clone, PartialEq, AsChangeset)]
#[diesel(table_name = channels)]
pub struct Channel {
    pub user_channel_id: String,
    pub channel_id: Option<String>,
    pub inbound: i64,
    pub outbound: i64,
    pub funding_txid: Option<String>,
    pub channel_state: ChannelState,
    pub counterparty_pubkey: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub liquidity_option_id: Option<i32>,
    pub fee_sats: Option<i64>,
    pub open_channel_payment_hash: Option<String>,
}

impl Channel {
    pub fn get(user_channel_id: &str, conn: &mut SqliteConnection) -> QueryResult<Option<Channel>> {
        channels::table
            .filter(schema::channels::user_channel_id.eq(user_channel_id))
            .first(conn)
            .optional()
    }

    pub fn get_announced_channel(
        conn: &mut SqliteConnection,
        counterparty_pubkey: &str,
    ) -> QueryResult<Option<Channel>> {
        channels::table
            .filter(schema::channels::counterparty_pubkey.eq(counterparty_pubkey))
            .filter(schema::channels::channel_state.eq(ChannelState::Announced))
            .first(conn)
            .optional()
    }
    pub fn get_by_channel_id(
        conn: &mut SqliteConnection,
        channel_id: &str,
    ) -> QueryResult<Option<Channel>> {
        channels::table
            .filter(schema::channels::channel_id.eq(channel_id))
            .first(conn)
            .optional()
    }

    pub fn get_channel_by_payment_hash(
        conn: &mut SqliteConnection,
        payment_hash: &str,
    ) -> QueryResult<Option<Channel>> {
        channels::table
            .filter(schema::channels::open_channel_payment_hash.eq(payment_hash))
            .first(conn)
            .optional()
    }

    pub fn get_all(conn: &mut SqliteConnection) -> QueryResult<Vec<Channel>> {
        channels::table.load(conn)
    }

    pub fn get_all_non_pending_channels(conn: &mut SqliteConnection) -> QueryResult<Vec<Channel>> {
        channels::table
            .filter(
                schema::channels::channel_state
                    .ne(ChannelState::Pending)
                    .and(schema::channels::funding_txid.is_not_null()),
            )
            .load(conn)
    }

    pub fn upsert(channel: Channel, conn: &mut SqliteConnection) -> Result<()> {
        let affected_rows = diesel::insert_into(channels::table)
            .values(channel.clone())
            .on_conflict(schema::channels::user_channel_id)
            .do_update()
            .set(&channel)
            .execute(conn)?;

        ensure!(affected_rows > 0, "Could not upsert channel");

        Ok(())
    }
}

#[derive(Insertable, QueryableByName, Queryable, Debug, Clone, PartialEq, AsChangeset)]
#[diesel(table_name = transactions)]
pub(crate) struct Transaction {
    pub txid: String,
    pub fee: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub raw: String,
}

impl Transaction {
    pub fn get(txid: &str, conn: &mut SqliteConnection) -> QueryResult<Option<Transaction>> {
        transactions::table
            .filter(transactions::txid.eq(txid))
            .first(conn)
            .optional()
    }

    pub fn get_all_without_fees(conn: &mut SqliteConnection) -> QueryResult<Vec<Transaction>> {
        transactions::table
            .filter(transactions::fee.eq(0))
            .load(conn)
    }

    pub fn upsert(tx: Transaction, conn: &mut SqliteConnection) -> Result<()> {
        let affected_rows = diesel::insert_into(transactions::table)
            .values(tx.clone())
            .on_conflict(schema::transactions::txid)
            .do_update()
            .set(&tx)
            .execute(conn)?;

        ensure!(affected_rows > 0, "Could not upsert transaction");

        Ok(())
    }
}

impl From<ln_dlc_node::transaction::Transaction> for Transaction {
    fn from(value: ln_dlc_node::transaction::Transaction) -> Self {
        Transaction {
            txid: value.txid().to_string(),
            fee: value.fee() as i64,
            created_at: value.created_at().unix_timestamp(),
            updated_at: value.updated_at().unix_timestamp(),
            raw: value.raw(),
        }
    }
}

impl From<Transaction> for ln_dlc_node::transaction::Transaction {
    fn from(value: Transaction) -> Self {
        ln_dlc_node::transaction::Transaction::new(
            Txid::from_str(&value.txid).expect("valid txid"),
            value.fee as u64,
            OffsetDateTime::from_unix_timestamp(value.created_at).expect("valid timestamp"),
            OffsetDateTime::from_unix_timestamp(value.updated_at).expect("valid timestamp"),
            value.raw,
        )
    }
}

#[derive(Insertable, Debug, Clone, PartialEq)]
#[diesel(table_name = trades)]
pub struct NewTrade {
    pub order_id: String,
    pub contract_symbol: ContractSymbol,
    pub contracts: f32,
    pub direction: Direction,
    pub trade_cost_sat: i64,
    pub fee_sat: i64,
    pub pnl_sat: Option<i64>,
    pub price: f32,
    pub timestamp: i64,
}

#[derive(Queryable, Debug, Clone, PartialEq)]
#[diesel(table_name = trades)]
pub struct Trade {
    pub id: i32,
    pub order_id: String,
    pub contract_symbol: ContractSymbol,
    pub contracts: f32,
    pub direction: Direction,
    pub trade_cost_sat: i64,
    pub fee_sat: i64,
    pub pnl_sat: Option<i64>,
    pub price: f32,
    pub timestamp: i64,
}

impl Trade {
    pub fn get_all(conn: &mut SqliteConnection) -> QueryResult<Vec<Self>> {
        trades::table.load(conn)
    }
}

impl NewTrade {
    pub fn insert(conn: &mut SqliteConnection, trade: Self) -> Result<()> {
        let affected_rows = diesel::insert_into(trades::table)
            .values(trade)
            .execute(conn)?;

        ensure!(affected_rows > 0, "Could not insert trade");

        Ok(())
    }
}

impl From<crate::trade::Trade> for NewTrade {
    fn from(value: crate::trade::Trade) -> Self {
        Self {
            order_id: value.order_id.to_string(),
            contract_symbol: value.contract_symbol.into(),
            contracts: value.contracts.to_f32().expect("contracts to fit into f32"),
            direction: value.direction.into(),
            trade_cost_sat: value.trade_cost.to_sat(),
            fee_sat: value.fee.to_sat() as i64,
            pnl_sat: value.pnl.map(|pnl| pnl.to_sat()),
            price: value.price.to_f32().expect("price to fit into f32"),
            timestamp: value.timestamp.unix_timestamp(),
        }
    }
}

impl From<Trade> for crate::trade::Trade {
    fn from(value: Trade) -> Self {
        Self {
            order_id: Uuid::parse_str(value.order_id.as_str()).expect("valid UUID"),
            contract_symbol: value.contract_symbol.into(),
            contracts: Decimal::from_f32(value.contracts).expect("contracts to fit into Decimal"),
            direction: value.direction.into(),
            trade_cost: SignedAmount::from_sat(value.trade_cost_sat),
            fee: Amount::from_sat(value.fee_sat as u64),
            pnl: value.pnl_sat.map(SignedAmount::from_sat),
            price: Decimal::from_f32(value.price).expect("price to fit into Decimal"),
            timestamp: OffsetDateTime::from_unix_timestamp(value.timestamp)
                .expect("valid UNIX timestamp"),
        }
    }
}

#[cfg(test)]
pub mod test {
    use super::*;
    use crate::db::MIGRATIONS;
    use crate::trade::order::FailureReason;
    use bitcoin::Txid;
    use diesel::result::Error;
    use diesel::Connection;
    use diesel::SqliteConnection;
    use diesel_migrations::MigrationHarness;
    use std::str::FromStr;
    use time::OffsetDateTime;
    use time::Time;

    #[test]
    fn order_round_trip() {
        let mut connection = SqliteConnection::establish(":memory:").unwrap();
        connection.run_pending_migrations(MIGRATIONS).unwrap();

        let uuid = uuid::Uuid::new_v4();
        let leverage = 2.0;
        let quantity = 100.0;
        let contract_symbol = trade::ContractSymbol::BtcUsd;
        let direction = trade::Direction::Long;
        let (order_type, limit_price) = crate::trade::order::OrderType::Market.into();
        let (status, execution_price, failure_reason) =
            crate::trade::order::OrderState::Initial.into();
        let creation_timestamp = OffsetDateTime::UNIX_EPOCH;
        let expiry_timestamp = OffsetDateTime::UNIX_EPOCH;

        let order = Order {
            id: uuid.to_string(),
            leverage,
            quantity,
            contract_symbol: contract_symbol.into(),
            direction: direction.into(),
            order_type,
            state: status,
            creation_timestamp: creation_timestamp.unix_timestamp(),
            limit_price,
            execution_price,
            failure_reason,
            order_expiry_timestamp: expiry_timestamp.unix_timestamp(),
            reason: OrderReason::Manual,
            stable: false,
            matching_fee_sats: None,
        };

        Order::insert(
            crate::trade::order::Order {
                id: uuid,
                leverage,
                quantity,
                contract_symbol,
                direction,
                order_type: crate::trade::order::OrderType::Market,
                state: crate::trade::order::OrderState::Initial,
                creation_timestamp,
                order_expiry_timestamp: expiry_timestamp,
                reason: crate::trade::order::OrderReason::Manual,
                stable: false,
                failure_reason: None,
            }
            .into(),
            &mut connection,
        )
        .unwrap();

        // Insert another one, just so that there is not just one order in the db
        Order::insert(
            crate::trade::order::Order {
                id: uuid::Uuid::new_v4(),
                leverage,
                quantity,
                contract_symbol,
                direction: trade::Direction::Long,
                order_type: crate::trade::order::OrderType::Market,
                state: crate::trade::order::OrderState::Initial,
                creation_timestamp,
                order_expiry_timestamp: expiry_timestamp,
                reason: crate::trade::order::OrderReason::Manual,
                stable: false,
                failure_reason: None,
            }
            .into(),
            &mut connection,
        )
        .unwrap();

        // load the order to see if it was randomly changed
        let loaded_order = Order::get(uuid.to_string(), &mut connection).unwrap();
        assert_eq!(order, loaded_order.unwrap());

        Order::update_state(
            uuid.to_string(),
            OrderState::Filled,
            Some(100000.0),
            None,
            None,
            &mut connection,
        )
        .unwrap();

        let updated_order = Order {
            state: OrderState::Filled,
            execution_price: Some(100000.0),
            ..order
        };

        let loaded_order = Order::get(uuid.to_string(), &mut connection).unwrap();
        assert_eq!(updated_order, loaded_order.unwrap());

        // delete it
        let deleted_rows = Order::delete(uuid.to_string(), &mut connection).unwrap();
        assert_eq!(deleted_rows, 1);

        // check if it is really gone
        match Order::get(uuid.to_string(), &mut connection).unwrap() {
            None => { // all good
            }
            Some(_) => {
                panic!("Expected to not being able to find said order")
            }
        }
    }

    #[test]
    fn given_several_orders_when_fetching_orders_for_ui_only_relevant_orders_are_loaded() {
        let mut connection = SqliteConnection::establish(":memory:").unwrap();
        connection.run_pending_migrations(MIGRATIONS).unwrap();

        let uuid = uuid::Uuid::new_v4();
        let leverage = 2.0;
        let quantity = 100.0;
        let contract_symbol = trade::ContractSymbol::BtcUsd;
        let direction = trade::Direction::Long;
        let creation_timestamp = OffsetDateTime::UNIX_EPOCH;
        let order_expiry_timestamp = OffsetDateTime::UNIX_EPOCH;

        Order::insert(
            crate::trade::order::Order {
                id: uuid,
                leverage,
                quantity,
                contract_symbol,
                direction,
                order_type: crate::trade::order::OrderType::Market,
                state: crate::trade::order::OrderState::Initial,
                creation_timestamp,
                order_expiry_timestamp,
                reason: crate::trade::order::OrderReason::Manual,
                stable: false,
                failure_reason: None,
            }
            .into(),
            &mut connection,
        )
        .unwrap();

        let orders = Order::get_without_rejected_and_initial(&mut connection).unwrap();
        assert_eq!(orders.len(), 0);

        let uuid1 = uuid::Uuid::new_v4();
        Order::insert(
            crate::trade::order::Order {
                id: uuid1,
                leverage,
                quantity,
                contract_symbol,
                direction,
                order_type: crate::trade::order::OrderType::Market,
                state: crate::trade::order::OrderState::Initial,
                creation_timestamp,
                order_expiry_timestamp,
                reason: crate::trade::order::OrderReason::Manual,
                stable: false,
                failure_reason: None,
            }
            .into(),
            &mut connection,
        )
        .unwrap();

        let orders = Order::get_without_rejected_and_initial(&mut connection).unwrap();
        assert_eq!(orders.len(), 0);

        Order::update_state(
            uuid.to_string(),
            OrderState::Open,
            None,
            None,
            None,
            &mut connection,
        )
        .unwrap();

        let orders = Order::get_without_rejected_and_initial(&mut connection).unwrap();
        assert_eq!(orders.len(), 1);

        Order::update_state(
            uuid1.to_string(),
            OrderState::Open,
            None,
            None,
            None,
            &mut connection,
        )
        .unwrap();

        let orders = Order::get_without_rejected_and_initial(&mut connection).unwrap();
        assert_eq!(orders.len(), 2);

        Order::update_state(
            uuid1.to_string(),
            OrderState::Failed,
            None,
            None,
            Some(FailureReason::FailedToSetToFilling.into()),
            &mut connection,
        )
        .unwrap();

        let orders = Order::get_without_rejected_and_initial(&mut connection).unwrap();
        assert_eq!(orders.len(), 2);
    }

    #[test]
    fn spendable_output_round_trip() {
        let mut connection = SqliteConnection::establish(":memory:").unwrap();
        connection.run_pending_migrations(MIGRATIONS).unwrap();

        let outpoint = lightning::chain::transaction::OutPoint {
            txid: bitcoin_old::hash_types::Txid::from_str(
                "219fede5479a69d8fc42693ecb8cea67098531087c421b4421d96e2f5acd7de3",
            )
            .unwrap(),
            index: 2,
        };
        let descriptor = lightning::sign::SpendableOutputDescriptor::StaticOutput {
            outpoint,
            output: bitcoin_old::TxOut {
                value: 10_000,
                script_pubkey: bitcoin_old::Script::new(),
            },
        };

        let spendable_output = (outpoint, descriptor.clone()).into();
        SpendableOutputInsertable::insert(spendable_output, &mut connection).unwrap();

        // Insert a random spendable output to show that we don't get confused with this one
        SpendableOutputInsertable::insert(
            {
                let outpoint = lightning::chain::transaction::OutPoint {
                    txid: bitcoin_old::hash_types::Txid::from_str(
                        "d0a8d75b352d015b7cd29a06d62c0aa92919927eefe7d6d016d7d01c0b7333a5",
                    )
                    .unwrap(),
                    index: 0,
                };
                (
                    outpoint,
                    lightning::sign::SpendableOutputDescriptor::StaticOutput {
                        outpoint,
                        output: bitcoin_old::TxOut {
                            value: 25_000,
                            script_pubkey: bitcoin_old::Script::new(),
                        },
                    },
                )
                    .into()
            },
            &mut connection,
        )
        .unwrap();

        // Verify that we can load the right spendable output based on its outpoint
        let loaded = SpendableOutputQueryable::get(outpoint, &mut connection).unwrap();

        let expected = SpendableOutputQueryable {
            id: 1,
            outpoint: "219fede5479a69d8fc42693ecb8cea67098531087c421b4421d96e2f5acd7de3:2"
                .to_string(),
            descriptor: hex::encode(descriptor.encode()),
        };

        assert_eq!(expected, loaded);

        // Verify that we can delete the right spendable output based on its outpoint
        SpendableOutputQueryable::delete(outpoint, &mut connection).unwrap();

        match SpendableOutputQueryable::get(outpoint, &mut connection) {
            // Could not find spendable output as expected
            Err(Error::NotFound) => {}
            _ => {
                panic!("Expected to not be able to find deleted spendable output");
            }
        };
    }

    #[test]
    fn transaction_round_trip() {
        let mut connection = SqliteConnection::establish(":memory:").unwrap();
        connection.run_pending_migrations(MIGRATIONS).unwrap();

        let transaction = ln_dlc_node::transaction::Transaction::new(
            Txid::from_str("44fe3d70a3058eb1bef62e24379b4865ada8332f9ee30752cf606f37343461a0")
                .unwrap(),
            0,
            // we need to set the time manually as the nano seconds are not stored in sql.
            OffsetDateTime::now_utc().replace_time(Time::from_hms(0, 0, 0).unwrap()),
            OffsetDateTime::now_utc().replace_time(Time::from_hms(0, 0, 0).unwrap()),
            "0200...doesntmattermuch".to_string(),
        );

        Transaction::upsert(transaction.clone().into(), &mut connection).unwrap();

        // Verify that we can load the right transaction by the `txid`
        let loaded: ln_dlc_node::transaction::Transaction = Transaction::get(
            "44fe3d70a3058eb1bef62e24379b4865ada8332f9ee30752cf606f37343461a0",
            &mut connection,
        )
        .unwrap()
        .unwrap()
        .into();

        assert_eq!(transaction, loaded);

        let second_tx = ln_dlc_node::transaction::Transaction::new(
            Txid::from_str("44fe3d70a3058eb1bef62e24379b4865ada8332f9ee30752cf606f37343461a1")
                .unwrap(),
            1,
            OffsetDateTime::now_utc(),
            OffsetDateTime::now_utc(),
            "0200...doesntmattermuch".to_string(),
        );
        Transaction::upsert(second_tx.into(), &mut connection).unwrap();
        // Verify that we can load all transactions without fees
        let transactions = Transaction::get_all_without_fees(&mut connection).unwrap();
        assert_eq!(1, transactions.len())
    }
}
