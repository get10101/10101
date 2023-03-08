use crate::api;
use crate::schema;
use crate::schema::last_login;
use crate::schema::orders;
use anyhow::bail;
use anyhow::Result;
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
    #[error("A filled order has to have an execution price")]
    MissingExecutionPrice,
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
    pub leverage: f64,
    pub quantity: f64,
    pub contract_symbol: ContractSymbol,
    pub direction: Direction,
    pub order_type: OrderType,
    pub state: OrderState,
    pub limit_price: Option<f64>,
    pub execution_price: Option<f64>,
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
        status: (OrderState, Option<f64>),
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

            let effected_rows = diesel::update(orders::table)
                .filter(schema::orders::id.eq(order_id))
                .set(schema::orders::execution_price.eq(status.1))
                .execute(conn)?;

            if effected_rows > 0 {
                Ok(())
            } else {
                bail!("Could not update order")
            }
        })
    }

    pub fn get(order_id: String, conn: &mut SqliteConnection) -> QueryResult<Order> {
        orders::table
            .filter(schema::orders::id.eq(order_id))
            .first(conn)
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
        let (status, execution_price) = value.state.into();

        Order {
            id: value.id.to_string(),
            leverage: value.leverage,
            quantity: value.quantity,
            contract_symbol: value.contract_symbol.into(),
            direction: value.direction.into(),
            order_type,
            state: status,
            limit_price,
            execution_price,
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
            state: (value.state, value.execution_price).try_into()?,
        };

        Ok(order)
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

impl From<crate::trade::order::OrderType> for (OrderType, Option<f64>) {
    fn from(value: crate::trade::order::OrderType) -> Self {
        match value {
            crate::trade::order::OrderType::Market => (OrderType::Market, None),
            crate::trade::order::OrderType::Limit { price } => (OrderType::Limit, Some(price)),
        }
    }
}

impl TryFrom<(OrderType, Option<f64>)> for crate::trade::order::OrderType {
    type Error = Error;

    fn try_from(value: (OrderType, Option<f64>)) -> std::result::Result<Self, Self::Error> {
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
    Failed,
    Filled,
}

impl From<crate::trade::order::OrderState> for (OrderState, Option<f64>) {
    fn from(value: crate::trade::order::OrderState) -> Self {
        match value {
            crate::trade::order::OrderState::Initial => (OrderState::Initial, None),
            crate::trade::order::OrderState::Rejected => (OrderState::Rejected, None),
            crate::trade::order::OrderState::Open => (OrderState::Open, None),
            crate::trade::order::OrderState::Failed => (OrderState::Failed, None),
            crate::trade::order::OrderState::Filled { execution_price } => {
                (OrderState::Filled, Some(execution_price))
            }
        }
    }
}

impl TryFrom<(OrderState, Option<f64>)> for crate::trade::order::OrderState {
    type Error = Error;

    fn try_from(value: (OrderState, Option<f64>)) -> std::result::Result<Self, Self::Error> {
        let order_state = match value.0 {
            OrderState::Initial => crate::trade::order::OrderState::Initial,
            OrderState::Rejected => crate::trade::order::OrderState::Rejected,
            OrderState::Open => crate::trade::order::OrderState::Open,
            OrderState::Failed => crate::trade::order::OrderState::Failed,
            OrderState::Filled => match value.1 {
                None => return Err(Error::MissingExecutionPrice),
                Some(execution_price) => {
                    crate::trade::order::OrderState::Filled { execution_price }
                }
            },
        };

        Ok(order_state)
    }
}

#[cfg(test)]
pub mod test {
    use crate::db::models::LastLogin;
    use crate::db::models::Order;
    use crate::db::models::OrderState;
    use crate::db::MIGRATIONS;
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
        let (status, execution_price) = crate::trade::order::OrderState::Initial.into();
        let order = Order {
            id: uuid.to_string(),
            leverage,
            quantity,
            contract_symbol: contract_symbol.into(),
            direction: direction.into(),
            order_type,
            state: status,
            limit_price,
            execution_price,
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
}
