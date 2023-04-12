use crate::api;
use crate::schema;
use crate::schema::last_login;
use crate::schema::orders;
use crate::schema::payments;
use crate::schema::positions;
use anyhow::anyhow;
use anyhow::bail;
use anyhow::ensure;
use anyhow::Result;
use base64::Engine;
use diesel;
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::Integer;
use diesel::sql_types::Text;
use diesel::AsExpression;
use diesel::FromSqlRow;
use diesel::Queryable;
use time::format_description;
use time::OffsetDateTime;
use trade;
use uuid::Uuid;

const SQLITE_DATETIME_FMT: &str = "[year]-[month]-[day] [hour]:[minute]:[second] [offset_hour \
         sign:mandatory]:[offset_minute]:[offset_second]";

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Invalid id when converting string to uuid")]
    InvalidId(#[from] uuid::Error),
    #[error("Limit order has to have a price")]
    MissingPriceForLimitOrder,
    #[error("A filling or filled order has to have an execution price")]
    MissingExecutionPrice,
    #[error("A failed order must have a reason")]
    MissingFailureReason,
}

#[derive(Queryable, QueryableByName, Debug, Clone)]
#[diesel(table_name = last_login)]
pub(crate) struct LastLogin {
    #[diesel(sql_type = Integer)]
    pub id: i32,
    #[diesel(sql_type = Text)]
    pub date: String,
}

impl From<LastLogin> for api::LastLogin {
    fn from(value: LastLogin) -> Self {
        api::LastLogin {
            id: value.id,
            date: value.date,
        }
    }
}

#[derive(Insertable, Debug, Clone)]
#[diesel(table_name = last_login)]
pub(crate) struct NewLastLogin {
    #[diesel(sql_type = Integer)]
    pub id: i32,
    #[diesel(sql_type = Text)]
    pub date: String,
}

impl LastLogin {
    /// Updates the timestamp the user logged in for the last time, returns the one before if
    /// successful
    pub fn update_last_login(
        last_login: OffsetDateTime,
        conn: &mut SqliteConnection,
    ) -> QueryResult<LastLogin> {
        let old_login = sql_query(
            "SELECT
                    id, date
                    FROM
                    last_login order by id",
        )
        .load::<LastLogin>(conn)?;
        let maybe_last_login = old_login.get(0).cloned();

        let format = format_description::parse(SQLITE_DATETIME_FMT).unwrap();

        let date = last_login.format(&format).unwrap();
        diesel::insert_into(last_login::table)
            .values(&NewLastLogin {
                id: 1,
                date: date.clone(),
            })
            .on_conflict(schema::last_login::id)
            .do_update()
            .set(schema::last_login::date.eq(date.clone()))
            .execute(conn)?;

        let login = maybe_last_login.unwrap_or(LastLogin { id: 1, date });
        Ok(login)
    }
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
}

impl Order {
    /// inserts the given order into the db. Returns the order if successful
    pub fn insert(order: Order, conn: &mut SqliteConnection) -> Result<Order> {
        let effected_rows = diesel::insert_into(orders::table)
            .values(&order)
            .execute(conn)?;

        if effected_rows > 0 {
            Ok(order)
        } else {
            bail!("Could not insert order")
        }
    }

    /// updates the status of the given order in the db
    pub fn update_state(
        order_id: String,
        status: (OrderState, Option<f32>, Option<FailureReason>),
        conn: &mut SqliteConnection,
    ) -> Result<()> {
        conn.transaction::<(), _, _>(|conn| {
            let effected_rows = diesel::update(orders::table)
                .filter(schema::orders::id.eq(order_id.clone()))
                .set(schema::orders::state.eq(status.0))
                .execute(conn)?;

            if effected_rows == 0 {
                bail!("Could not update order")
            }

            if let Some(execution_price) = status.1 {
                diesel::update(orders::table)
                    .filter(schema::orders::id.eq(order_id.clone()))
                    .set(schema::orders::execution_price.eq(execution_price))
                    .execute(conn)?;

                if effected_rows == 0 {
                    bail!("Could not update order")
                }
            }

            if let Some(failure_reason) = status.2 {
                diesel::update(orders::table)
                    .filter(schema::orders::id.eq(order_id))
                    .set(schema::orders::failure_reason.eq(failure_reason))
                    .execute(conn)?;

                if effected_rows == 0 {
                    bail!("Could not update order")
                }
            }

            Ok(())
        })
    }

    pub fn get(order_id: String, conn: &mut SqliteConnection) -> QueryResult<Order> {
        orders::table
            .filter(schema::orders::id.eq(order_id))
            .first(conn)
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
        let (status, execution_price, failure_reason) = value.state.into();

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
            state: (value.state, value.execution_price, value.failure_reason).try_into()?,
            creation_timestamp: OffsetDateTime::from_unix_timestamp(value.creation_timestamp)
                .expect("unix timestamp to fit in itself"),
        };

        Ok(order)
    }
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
}

#[derive(Debug, Clone, Copy, PartialEq, FromSqlRow, AsExpression)]
#[diesel(sql_type = Text)]
pub enum PositionState {
    Open,
    Closing,
}

impl Position {
    /// inserts the given position into the db. Returns the position if successful
    pub fn insert(position: Position, conn: &mut SqliteConnection) -> Result<Position> {
        let effected_rows = diesel::insert_into(positions::table)
            .values(&position)
            .execute(conn)?;

        if effected_rows > 0 {
            Ok(position)
        } else {
            bail!("Could not insert position")
        }
    }

    pub fn get_all(conn: &mut SqliteConnection) -> QueryResult<Vec<Position>> {
        positions::table.load(conn)
    }

    /// updates the status of the given order in the db
    pub fn update_state(
        contract_symbol: ContractSymbol,
        state: PositionState,
        conn: &mut SqliteConnection,
    ) -> Result<()> {
        let effected_rows = diesel::update(positions::table)
            .filter(schema::positions::contract_symbol.eq(contract_symbol))
            .set(schema::positions::state.eq(state))
            .execute(conn)?;

        if effected_rows == 0 {
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
        }
    }
}

impl From<crate::trade::position::PositionState> for PositionState {
    fn from(value: crate::trade::position::PositionState) -> Self {
        match value {
            crate::trade::position::PositionState::Open => PositionState::Open,
            crate::trade::position::PositionState::Closing => PositionState::Closing,
        }
    }
}

impl From<PositionState> for crate::trade::position::PositionState {
    fn from(value: PositionState) -> Self {
        match value {
            PositionState::Open => crate::trade::position::PositionState::Open,
            PositionState::Closing => crate::trade::position::PositionState::Closing,
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
pub enum OrderState {
    Initial,
    Rejected,
    Open,
    Filling,
    Failed,
    Filled,
}

impl From<crate::trade::order::OrderState> for (OrderState, Option<f32>, Option<FailureReason>) {
    fn from(value: crate::trade::order::OrderState) -> Self {
        match value {
            crate::trade::order::OrderState::Initial => (OrderState::Initial, None, None),
            crate::trade::order::OrderState::Rejected => (OrderState::Rejected, None, None),
            crate::trade::order::OrderState::Open => (OrderState::Open, None, None),
            crate::trade::order::OrderState::Failed { reason } => {
                (OrderState::Failed, None, Some(reason.into()))
            }
            crate::trade::order::OrderState::Filled { execution_price } => {
                (OrderState::Filled, Some(execution_price), None)
            }
            crate::trade::order::OrderState::Filling { execution_price } => {
                (OrderState::Filling, Some(execution_price), None)
            }
        }
    }
}

impl TryFrom<(OrderState, Option<f32>, Option<FailureReason>)> for crate::trade::order::OrderState {
    type Error = Error;

    fn try_from(
        value: (OrderState, Option<f32>, Option<FailureReason>),
    ) -> std::result::Result<Self, Self::Error> {
        let order_state = match value.0 {
            OrderState::Initial => crate::trade::order::OrderState::Initial,
            OrderState::Rejected => crate::trade::order::OrderState::Rejected,
            OrderState::Open => crate::trade::order::OrderState::Open,
            OrderState::Failed => match value.2 {
                None => return Err(Error::MissingFailureReason),
                Some(reason) => crate::trade::order::OrderState::Failed {
                    reason: reason.into(),
                },
            },
            OrderState::Filled => match value.1 {
                None => return Err(Error::MissingExecutionPrice),
                Some(execution_price) => {
                    crate::trade::order::OrderState::Filled { execution_price }
                }
            },
            OrderState::Filling => match value.1 {
                None => return Err(Error::MissingExecutionPrice),
                Some(execution_price) => {
                    crate::trade::order::OrderState::Filling { execution_price }
                }
            },
        };

        Ok(order_state)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, FromSqlRow, AsExpression)]
#[diesel(sql_type = Text)]
pub enum FailureReason {
    FailedToSetToFilling,
    TradeRequest,
    TradeResponse,
    NodeAccess,
    NoUsableChannel,
    ProposeDlcChannel,
    OrderNotAcceptable,
}

impl From<FailureReason> for crate::trade::order::FailureReason {
    fn from(value: FailureReason) -> Self {
        match value {
            FailureReason::TradeRequest => crate::trade::order::FailureReason::TradeRequest,
            FailureReason::TradeResponse => crate::trade::order::FailureReason::TradeResponse,
            FailureReason::NodeAccess => crate::trade::order::FailureReason::NodeAccess,
            FailureReason::NoUsableChannel => crate::trade::order::FailureReason::NoUsableChannel,
            FailureReason::ProposeDlcChannel => {
                crate::trade::order::FailureReason::ProposeDlcChannel
            }
            FailureReason::FailedToSetToFilling => {
                crate::trade::order::FailureReason::FailedToSetToFilling
            }
            FailureReason::OrderNotAcceptable => {
                crate::trade::order::FailureReason::OrderNotAcceptable
            }
        }
    }
}

impl From<crate::trade::order::FailureReason> for FailureReason {
    fn from(value: crate::trade::order::FailureReason) -> Self {
        match value {
            crate::trade::order::FailureReason::TradeRequest => FailureReason::TradeRequest,
            crate::trade::order::FailureReason::TradeResponse => FailureReason::TradeResponse,
            crate::trade::order::FailureReason::NodeAccess => FailureReason::NodeAccess,
            crate::trade::order::FailureReason::NoUsableChannel => FailureReason::NoUsableChannel,
            crate::trade::order::FailureReason::ProposeDlcChannel => {
                FailureReason::ProposeDlcChannel
            }
            crate::trade::order::FailureReason::FailedToSetToFilling => {
                FailureReason::FailedToSetToFilling
            }
            crate::trade::order::FailureReason::OrderNotAcceptable => {
                FailureReason::OrderNotAcceptable
            }
        }
    }
}

#[derive(Insertable, Debug, Clone, PartialEq)]
#[diesel(table_name = payments)]
pub(crate) struct PaymentInsertable {
    #[diesel(sql_type = Text)]
    pub payment_hash: String,
    #[diesel(sql_type = Nullabel<Text>)]
    pub preimage: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    pub secret: Option<String>,
    pub htlc_status: HtlcStatus,
    #[diesel(sql_type = Nullable<BigInt>)]
    pub amount_msat: Option<i64>,
    pub flow: Flow,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, FromSqlRow, AsExpression)]
#[diesel(sql_type = Text)]
pub enum HtlcStatus {
    Pending,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, FromSqlRow, AsExpression)]
#[diesel(sql_type = Text)]
pub enum Flow {
    Inbound,
    Outbound,
}

impl PaymentInsertable {
    pub fn insert(payment: PaymentInsertable, conn: &mut SqliteConnection) -> Result<()> {
        let effected_rows = diesel::insert_into(payments::table)
            .values(&payment)
            .execute(conn)?;

        ensure!(effected_rows > 0, "Could not insert payment");

        Ok(())
    }

    pub fn update(
        payment_hash: String,
        htlc_status: HtlcStatus,
        amount_msat: Option<i64>,
        preimage: Option<String>,
        secret: Option<String>,
        conn: &mut SqliteConnection,
    ) -> Result<i64> {
        let updated_at = OffsetDateTime::now_utc().unix_timestamp();

        conn.transaction::<(), _, _>(|conn| {
            let effected_rows = diesel::update(payments::table)
                .filter(schema::payments::payment_hash.eq(&payment_hash))
                .set(schema::payments::htlc_status.eq(htlc_status))
                .execute(conn)?;

            if effected_rows == 0 {
                bail!("Could not update payment HTLC status")
            }

            if let Some(amount_msat) = amount_msat {
                let effected_rows = diesel::update(payments::table)
                    .filter(schema::payments::payment_hash.eq(&payment_hash))
                    .set(schema::payments::amount_msat.eq(amount_msat))
                    .execute(conn)?;

                if effected_rows == 0 {
                    bail!("Could not update payment amount")
                }
            }

            if let Some(preimage) = preimage {
                let effected_rows = diesel::update(payments::table)
                    .filter(schema::payments::payment_hash.eq(&payment_hash))
                    .set(schema::payments::preimage.eq(preimage))
                    .execute(conn)?;

                if effected_rows == 0 {
                    bail!("Could not update payment preimage")
                }
            }

            if let Some(secret) = secret {
                let effected_rows = diesel::update(payments::table)
                    .filter(schema::payments::payment_hash.eq(&payment_hash))
                    .set(schema::payments::secret.eq(secret))
                    .execute(conn)?;

                if effected_rows == 0 {
                    bail!("Could not update payment secret")
                }
            }

            let effected_rows = diesel::update(payments::table)
                .filter(schema::payments::payment_hash.eq(&payment_hash))
                .set(schema::payments::updated_at.eq(updated_at))
                .execute(conn)?;

            if effected_rows == 0 {
                bail!("Could not update payment updated_at xtimestamp")
            }

            Ok(())
        })?;

        Ok(updated_at)
    }
}

#[derive(Queryable, Debug, Clone, PartialEq)]
#[diesel(table_name = payments)]
pub(crate) struct PaymentQueryable {
    pub id: i32,
    pub payment_hash: String,
    pub preimage: Option<String>,
    pub secret: Option<String>,
    pub htlc_status: HtlcStatus,
    pub amount_msat: Option<i64>,
    pub flow: Flow,
    pub created_at: i64,
    pub updated_at: i64,
}

impl PaymentQueryable {
    pub fn get(payment_hash: String, conn: &mut SqliteConnection) -> QueryResult<PaymentQueryable> {
        payments::table
            .filter(schema::payments::payment_hash.eq(payment_hash))
            .first(conn)
    }

    pub fn get_all(conn: &mut SqliteConnection) -> QueryResult<Vec<PaymentQueryable>> {
        payments::table.load(conn)
    }
}

impl From<(lightning::ln::PaymentHash, ln_dlc_node::PaymentInfo)> for PaymentInsertable {
    fn from((payment_hash, info): (lightning::ln::PaymentHash, ln_dlc_node::PaymentInfo)) -> Self {
        let base64 = base64_engine();

        let timestamp = info.timestamp.unix_timestamp();

        Self {
            payment_hash: base64.encode(payment_hash.0),
            preimage: info.preimage.map(|preimage| base64.encode(preimage.0)),
            secret: info.secret.map(|secret| base64.encode(secret.0)),
            htlc_status: info.status.into(),
            amount_msat: info.amt_msat.to_inner().map(|amt| amt as i64),
            flow: info.flow.into(),
            created_at: timestamp,
            updated_at: timestamp,
        }
    }
}

impl TryFrom<PaymentQueryable> for (lightning::ln::PaymentHash, ln_dlc_node::PaymentInfo) {
    type Error = anyhow::Error;

    fn try_from(value: PaymentQueryable) -> Result<Self> {
        let base64 = base64_engine();

        let payment_hash = base64.decode(value.payment_hash)?;
        let payment_hash = payment_hash
            .try_into()
            .map_err(|_| anyhow!("Can't convert payment hash to array"))?;
        let payment_hash = lightning::ln::PaymentHash(payment_hash);

        let preimage = value
            .preimage
            .map(|preimage| {
                let preimage = base64.decode(preimage)?;
                let preimage = preimage
                    .try_into()
                    .map_err(|_| anyhow!("Can't convert preimage to array"))?;

                anyhow::Ok(lightning::ln::PaymentPreimage(preimage))
            })
            .transpose()?;

        let secret = value
            .secret
            .map(|secret| {
                let secret = base64.decode(secret)?;
                let secret = secret
                    .try_into()
                    .map_err(|_| anyhow!("Can't convert secret to array"))?;

                anyhow::Ok(lightning::ln::PaymentSecret(secret))
            })
            .transpose()?;

        let status = value.htlc_status.into();

        let amt_msat =
            ln_dlc_node::MillisatAmount::new(value.amount_msat.map(|amount| amount as u64));

        let flow = value.flow.into();

        let timestamp = OffsetDateTime::from_unix_timestamp(value.created_at)?;

        Ok((
            payment_hash,
            ln_dlc_node::PaymentInfo {
                preimage,
                secret,
                status,
                amt_msat,
                flow,
                timestamp,
            },
        ))
    }
}

impl From<ln_dlc_node::HTLCStatus> for HtlcStatus {
    fn from(value: ln_dlc_node::HTLCStatus) -> Self {
        match value {
            ln_dlc_node::node::HTLCStatus::Pending => Self::Pending,
            ln_dlc_node::node::HTLCStatus::Succeeded => Self::Succeeded,
            ln_dlc_node::node::HTLCStatus::Failed => Self::Failed,
        }
    }
}

impl From<HtlcStatus> for ln_dlc_node::HTLCStatus {
    fn from(value: HtlcStatus) -> Self {
        match value {
            HtlcStatus::Pending => Self::Pending,
            HtlcStatus::Succeeded => Self::Succeeded,
            HtlcStatus::Failed => Self::Failed,
        }
    }
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

pub(crate) fn base64_engine() -> base64::engine::GeneralPurpose {
    base64::engine::GeneralPurpose::new(
        &base64::alphabet::STANDARD,
        base64::engine::GeneralPurposeConfig::new(),
    )
}

#[cfg(test)]
pub mod test {
    use crate::db::models::Flow;
    use crate::db::models::HtlcStatus;
    use crate::db::models::LastLogin;
    use crate::db::models::Order;
    use crate::db::models::OrderState;
    use crate::db::models::PaymentInsertable;
    use crate::db::models::PaymentQueryable;
    use crate::db::MIGRATIONS;
    use crate::trade::order::FailureReason;
    use diesel::result::Error;
    use diesel::Connection;
    use diesel::SqliteConnection;
    use diesel_migrations::MigrationHarness;
    use time::OffsetDateTime;

    #[test]
    pub fn when_no_login_return_input() {
        let mut connection = SqliteConnection::establish(":memory:").unwrap();
        connection.run_pending_migrations(MIGRATIONS).unwrap();

        let back_in_the_days = OffsetDateTime::UNIX_EPOCH;
        let last_login = LastLogin::update_last_login(back_in_the_days, &mut connection).unwrap();

        assert_eq!("1970-01-01 00:00:00 +00:00:00".to_string(), last_login.date);
    }

    #[test]
    pub fn when_already_logged_in_return_former_login() {
        let mut connection = SqliteConnection::establish(":memory:").unwrap();
        connection.run_pending_migrations(MIGRATIONS).unwrap();

        let back_in_the_days = OffsetDateTime::UNIX_EPOCH;
        let _ = LastLogin::update_last_login(back_in_the_days, &mut connection).unwrap();

        let back_in_the_days_as_well_but_10_secs_later =
            OffsetDateTime::from_unix_timestamp(back_in_the_days.unix_timestamp() + 10).unwrap();
        let _ = LastLogin::update_last_login(
            back_in_the_days_as_well_but_10_secs_later,
            &mut connection,
        )
        .unwrap();

        let now = OffsetDateTime::now_utc();
        let last_login = LastLogin::update_last_login(now, &mut connection).unwrap();

        assert_eq!("1970-01-01 00:00:10 +00:00:00".to_string(), last_login.date);
    }

    #[test]
    pub fn order_round_trip() {
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
            }
            .into(),
            &mut connection,
        )
        .unwrap();

        // load the order to see if it was randomly changed
        let loaded_order = Order::get(uuid.to_string(), &mut connection).unwrap();
        assert_eq!(order, loaded_order);

        Order::update_state(
            uuid.to_string(),
            (crate::trade::order::OrderState::Filled {
                execution_price: 100000.0,
            })
            .into(),
            &mut connection,
        )
        .unwrap();

        let updated_order = Order {
            state: OrderState::Filled,
            execution_price: Some(100000.0),
            ..order
        };

        let loaded_order = Order::get(uuid.to_string(), &mut connection).unwrap();
        assert_eq!(updated_order, loaded_order);

        // delete it
        let deleted_rows = Order::delete(uuid.to_string(), &mut connection).unwrap();
        assert_eq!(deleted_rows, 1);

        // check if it is really gone
        match Order::get(uuid.to_string(), &mut connection) {
            Err(Error::NotFound) => { // all good
            }
            _ => {
                panic!("Expected to not being able to find said order")
            }
        }
    }

    #[test]
    pub fn given_several_orders_when_fetching_orders_for_ui_only_relevant_orders_are_loaded() {
        let mut connection = SqliteConnection::establish(":memory:").unwrap();
        connection.run_pending_migrations(MIGRATIONS).unwrap();

        let uuid = uuid::Uuid::new_v4();
        let leverage = 2.0;
        let quantity = 100.0;
        let contract_symbol = trade::ContractSymbol::BtcUsd;
        let direction = trade::Direction::Long;
        let creation_timestamp = OffsetDateTime::UNIX_EPOCH;

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
            }
            .into(),
            &mut connection,
        )
        .unwrap();

        let orders = Order::get_without_rejected_and_initial(&mut connection).unwrap();
        assert_eq!(orders.len(), 0);

        Order::update_state(
            uuid.to_string(),
            crate::trade::order::OrderState::Open.into(),
            &mut connection,
        )
        .unwrap();

        let orders = Order::get_without_rejected_and_initial(&mut connection).unwrap();
        assert_eq!(orders.len(), 1);

        Order::update_state(
            uuid1.to_string(),
            crate::trade::order::OrderState::Open.into(),
            &mut connection,
        )
        .unwrap();

        let orders = Order::get_without_rejected_and_initial(&mut connection).unwrap();
        assert_eq!(orders.len(), 2);

        Order::update_state(
            uuid1.to_string(),
            crate::trade::order::OrderState::Failed {
                reason: FailureReason::FailedToSetToFilling,
            }
            .into(),
            &mut connection,
        )
        .unwrap();

        let orders = Order::get_without_rejected_and_initial(&mut connection).unwrap();
        assert_eq!(orders.len(), 2);
    }

    #[test]
    pub fn payment_round_trip() {
        let mut connection = SqliteConnection::establish(":memory:").unwrap();
        connection.run_pending_migrations(MIGRATIONS).unwrap();

        let payment_hash = "lnbc2500u1pvjluezpp5qqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqypqdq5xysxxatsyp3k7enxv4jsxqzpuaztrnwngzn3kdzw5hydlzf03qdgm2hdq27cqv3agm2awhz5se903vruatfhq77w3ls4evs3ch9zw97j25emudupq63nyw24cg27h2rspfj9srp";
        let preimage = None;
        let secret = None;
        let htlc_status = HtlcStatus::Pending;
        let amount_msat = Some(10_000_000);
        let flow = Flow::Inbound;
        let created_at = 100;
        let updated_at = 100;

        let payment = PaymentInsertable {
            payment_hash: payment_hash.to_string(),
            preimage: preimage.clone(),
            secret: secret.clone(),
            htlc_status,
            amount_msat,
            flow,
            created_at,
            updated_at,
        };

        PaymentInsertable::insert(payment, &mut connection).unwrap();

        // Insert a random payment to show that we don't get confused with this one
        PaymentInsertable::insert(
            PaymentInsertable {
                payment_hash: "lnbcrt100u1pjzy0v3dq8w3jhxaqpp55s8w2jfqatnjh4ntsrv36nvzutp9wm25zqyrn9glrnxgfu72l3cqsp50rj4gs2ck2vjungtx9auetdfa5eeglw89c037nv3fcj03xtj0shs9qrsgqcqpcrzjqfhpmc88dypdw8fvy7lam2w53svuf32s7z9mgxyawgyzgmsw8tuhuqqqqyqq2tgqqvqqqqlgqqqyugqq9gefmmc3jhl85nhhq0cljg2muqsj4z54j770xym29h2mutzu6gg8d86p50wcuazxrzhr8lfen9htg605gj3hp86vedhp7a46ypdsrg34sqm5t9tv".to_string(),
                preimage: None,
                secret: None,
                htlc_status: HtlcStatus::Pending,
                amount_msat: None,
                flow: Flow::Outbound,
                created_at: 200,
                updated_at: 200,
            },
            &mut connection,
        )
        .unwrap();

        // Verify that we can load the right payment based on its payment_hash
        let loaded_payment =
            PaymentQueryable::get(payment_hash.to_string(), &mut connection).unwrap();

        let expected_payment = PaymentQueryable {
            id: 1,
            payment_hash: payment_hash.to_string(),
            preimage,
            secret,
            htlc_status,
            amount_msat,
            flow,
            created_at,
            updated_at,
        };

        assert_eq!(expected_payment, loaded_payment);

        // Verify that we can update the payment

        let new_htlc_status = HtlcStatus::Succeeded;
        let preimage = Some("preimage".to_string());
        let amount_msat = Some(1_000_000);
        let secret = Some("secret".to_string());

        let updated_at = PaymentInsertable::update(
            payment_hash.to_string(),
            new_htlc_status,
            amount_msat,
            preimage.clone(),
            secret.clone(),
            &mut connection,
        )
        .unwrap();

        let loaded_payment =
            PaymentQueryable::get(payment_hash.to_string(), &mut connection).unwrap();

        let expected_payment = PaymentQueryable {
            preimage,
            secret,
            htlc_status: new_htlc_status,
            amount_msat,
            updated_at,
            ..expected_payment
        };

        assert_eq!(expected_payment, loaded_payment);
    }
}
