use crate::schema::sql_types::DirectionType;
use crate::schema::sql_types::MatchStateType;
use crate::schema::sql_types::OrderReasonType;
use crate::schema::sql_types::OrderStateType;
use crate::schema::sql_types::OrderTypeType;
use diesel::deserialize;
use diesel::deserialize::FromSql;
use diesel::pg::Pg;
use diesel::pg::PgValue;
use diesel::query_builder::QueryId;
use diesel::serialize;
use diesel::serialize::IsNull;
use diesel::serialize::Output;
use diesel::serialize::ToSql;
use diesel::AsExpression;
use diesel::FromSqlRow;
use std::any::TypeId;
use std::io::Write;

#[derive(Debug, Clone, Copy, PartialEq, FromSqlRow, AsExpression, Eq)]
#[diesel(sql_type = DirectionType)]
pub enum Direction {
    Long,
    Short,
}

impl QueryId for DirectionType {
    type QueryId = DirectionType;
    const HAS_STATIC_QUERY_ID: bool = false;

    fn query_id() -> Option<TypeId> {
        None
    }
}

impl ToSql<DirectionType, Pg> for Direction {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        match *self {
            Direction::Long => out.write_all(b"long")?,
            Direction::Short => out.write_all(b"short")?,
        }
        Ok(IsNull::No)
    }
}

impl FromSql<DirectionType, Pg> for Direction {
    fn from_sql(bytes: PgValue<'_>) -> deserialize::Result<Self> {
        match bytes.as_bytes() {
            b"long" => Ok(Direction::Long),
            b"short" => Ok(Direction::Short),
            _ => Err("Unrecognized enum variant".into()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, FromSqlRow, AsExpression, Eq)]
#[diesel(sql_type = OrderTypeType)]
pub enum OrderType {
    Market,
    Limit,
}

impl QueryId for OrderTypeType {
    type QueryId = OrderTypeType;
    const HAS_STATIC_QUERY_ID: bool = false;

    fn query_id() -> Option<TypeId> {
        None
    }
}

impl ToSql<OrderTypeType, Pg> for OrderType {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        match *self {
            OrderType::Market => out.write_all(b"market")?,
            OrderType::Limit => out.write_all(b"limit")?,
        }
        Ok(IsNull::No)
    }
}

impl FromSql<OrderTypeType, Pg> for OrderType {
    fn from_sql(bytes: PgValue<'_>) -> deserialize::Result<Self> {
        match bytes.as_bytes() {
            b"market" => Ok(OrderType::Market),
            b"limit" => Ok(OrderType::Limit),
            _ => Err("Unrecognized enum variant".into()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, FromSqlRow, AsExpression)]
#[diesel(sql_type = OrderStateType)]
pub(crate) enum OrderState {
    /// The order is waiting for a match.
    Open,
    /// The order is matched, but the trade has not yet happened.
    Matched,
    /// The trade has been initiated for that order.
    Taken,
    /// The order failed.
    Failed,
    /// The order expired.
    Expired,
    /// The order was manually deleted by the trader
    Deleted,
}

impl QueryId for OrderStateType {
    type QueryId = OrderStateType;
    const HAS_STATIC_QUERY_ID: bool = false;

    fn query_id() -> Option<TypeId> {
        None
    }
}

impl ToSql<OrderStateType, Pg> for OrderState {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        match *self {
            OrderState::Open => out.write_all(b"Open")?,
            OrderState::Matched => out.write_all(b"Matched")?,
            OrderState::Taken => out.write_all(b"Taken")?,
            OrderState::Failed => out.write_all(b"Failed")?,
            OrderState::Expired => out.write_all(b"Expired")?,
            OrderState::Deleted => out.write_all(b"Deleted")?,
        }
        Ok(IsNull::No)
    }
}

impl FromSql<OrderStateType, Pg> for OrderState {
    fn from_sql(bytes: PgValue<'_>) -> deserialize::Result<Self> {
        match bytes.as_bytes() {
            b"Open" => Ok(OrderState::Open),
            b"Matched" => Ok(OrderState::Matched),
            b"Taken" => Ok(OrderState::Taken),
            b"Failed" => Ok(OrderState::Failed),
            b"Expired" => Ok(OrderState::Expired),
            b"Deleted" => Ok(OrderState::Deleted),
            _ => Err("Unrecognized enum variant".into()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, FromSqlRow, AsExpression)]
#[diesel(sql_type = OrderReasonType)]
pub(crate) enum OrderReason {
    /// The order has been created manually by the user.
    Manual,
    /// The order has been create automatically as the position expired.
    Expired,
    /// The order has been created automatically as the position got liquidated.
    TraderLiquidated,
    CoordinatorLiquidated,
}

impl QueryId for OrderReasonType {
    type QueryId = OrderReasonType;
    const HAS_STATIC_QUERY_ID: bool = false;

    fn query_id() -> Option<TypeId> {
        None
    }
}

impl ToSql<OrderReasonType, Pg> for OrderReason {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        match *self {
            OrderReason::Manual => out.write_all(b"Manual")?,
            OrderReason::Expired => out.write_all(b"Expired")?,
            OrderReason::TraderLiquidated => out.write_all(b"TraderLiquidated")?,
            OrderReason::CoordinatorLiquidated => out.write_all(b"CoordinatorLiquidated")?,
        }
        Ok(IsNull::No)
    }
}

impl FromSql<OrderReasonType, Pg> for OrderReason {
    fn from_sql(bytes: PgValue<'_>) -> deserialize::Result<Self> {
        match bytes.as_bytes() {
            b"Manual" => Ok(OrderReason::Manual),
            b"Expired" => Ok(OrderReason::Expired),
            b"TraderLiquidated" => Ok(OrderReason::TraderLiquidated),
            b"CoordinatorLiquidated" => Ok(OrderReason::CoordinatorLiquidated),
            _ => Err("Unrecognized enum variant".into()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, FromSqlRow, AsExpression)]
#[diesel(sql_type = MatchStateType)]
pub(crate) enum MatchState {
    Pending,
    Filled,
    Failed,
}

impl QueryId for MatchStateType {
    type QueryId = MatchStateType;
    const HAS_STATIC_QUERY_ID: bool = false;

    fn query_id() -> Option<TypeId> {
        None
    }
}

impl ToSql<MatchStateType, Pg> for MatchState {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        match *self {
            MatchState::Pending => out.write_all(b"Pending")?,
            MatchState::Filled => out.write_all(b"Filled")?,
            MatchState::Failed => out.write_all(b"Failed")?,
        }
        Ok(IsNull::No)
    }
}

impl FromSql<MatchStateType, Pg> for MatchState {
    fn from_sql(bytes: PgValue<'_>) -> deserialize::Result<Self> {
        match bytes.as_bytes() {
            b"Pending" => Ok(MatchState::Pending),
            b"Filled" => Ok(MatchState::Filled),
            b"Failed" => Ok(MatchState::Failed),
            _ => Err("Unrecognized enum variant".into()),
        }
    }
}
