use crate::db::positions::ContractSymbol;
use crate::db::positions::PositionState;
use crate::schema::sql_types::ContractSymbolType;
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
