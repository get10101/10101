use crate::db::payments::HtlcStatus;
use crate::db::payments::PaymentFlow;
use crate::db::positions::ContractSymbol;
use crate::db::positions::PositionState;
use crate::schema::sql_types::ContractSymbolType;
use crate::schema::sql_types::DirectionType;
use crate::schema::sql_types::HtlcStatusType;
use crate::schema::sql_types::PaymentFlowType;
use crate::schema::sql_types::PositionStateType;
use diesel::deserialize::FromSql;
use diesel::deserialize::{self};
use diesel::pg::Pg;
use diesel::pg::PgValue;
use diesel::serialize::IsNull;
use diesel::serialize::Output;
use diesel::serialize::ToSql;
use diesel::serialize::{self};
use std::io::Write;
use trade::Direction;

impl ToSql<ContractSymbolType, Pg> for ContractSymbol {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        match *self {
            ContractSymbol::BtcUsd => out.write_all(b"BtcUsd")?,
        }
        Ok(IsNull::No)
    }
}

impl FromSql<ContractSymbolType, Pg> for ContractSymbol {
    fn from_sql(bytes: PgValue<'_>) -> deserialize::Result<Self> {
        match bytes.as_bytes() {
            b"BtcUsd" => Ok(ContractSymbol::BtcUsd),
            _ => Err("Unrecognized enum variant".into()),
        }
    }
}

impl ToSql<PositionStateType, Pg> for PositionState {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        match *self {
            PositionState::Open => out.write_all(b"Open")?,
            PositionState::Closing => out.write_all(b"Closing")?,
        }
        Ok(IsNull::No)
    }
}

impl FromSql<PositionStateType, Pg> for PositionState {
    fn from_sql(bytes: PgValue<'_>) -> deserialize::Result<Self> {
        match bytes.as_bytes() {
            b"Open" => Ok(PositionState::Open),
            b"Closing" => Ok(PositionState::Closing),
            _ => Err("Unrecognized enum variant".into()),
        }
    }
}

impl ToSql<HtlcStatusType, Pg> for HtlcStatus {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        match *self {
            HtlcStatus::Pending => out.write_all(b"Pending")?,
            HtlcStatus::Succeeded => out.write_all(b"Succeeded")?,
            HtlcStatus::Failed => out.write_all(b"Failed")?,
        }
        Ok(IsNull::No)
    }
}

impl FromSql<HtlcStatusType, Pg> for HtlcStatus {
    fn from_sql(bytes: PgValue<'_>) -> deserialize::Result<Self> {
        match bytes.as_bytes() {
            b"Pending" => Ok(HtlcStatus::Pending),
            b"Succeeded" => Ok(HtlcStatus::Succeeded),
            b"Failed" => Ok(HtlcStatus::Failed),
            _ => Err("Unrecognized enum variant".into()),
        }
    }
}

impl ToSql<PaymentFlowType, Pg> for PaymentFlow {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        match *self {
            PaymentFlow::Inbound => out.write_all(b"Inbound")?,
            PaymentFlow::Outbound => out.write_all(b"Outbound")?,
        }
        Ok(IsNull::No)
    }
}

impl FromSql<PaymentFlowType, Pg> for PaymentFlow {
    fn from_sql(bytes: PgValue<'_>) -> deserialize::Result<Self> {
        match bytes.as_bytes() {
            b"Inbound" => Ok(PaymentFlow::Inbound),
            b"Outbound" => Ok(PaymentFlow::Outbound),
            _ => Err("Unrecognized enum variant".into()),
        }
    }
}

impl ToSql<DirectionType, Pg> for Direction {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        match *self {
            Direction::Long => out.write_all(b"Long")?,
            Direction::Short => out.write_all(b"Short")?,
        }
        Ok(IsNull::No)
    }
}

impl FromSql<DirectionType, Pg> for Direction {
    fn from_sql(bytes: PgValue<'_>) -> deserialize::Result<Self> {
        match bytes.as_bytes() {
            b"Long" => Ok(Direction::Long),
            b"Short" => Ok(Direction::Short),
            _ => Err("Unrecognized enum variant".into()),
        }
    }
}
