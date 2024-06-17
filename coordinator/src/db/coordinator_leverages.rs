use crate::schema::coordinator_leverages;
use anyhow::Result;
use diesel::dsl::max;
use diesel::ExpressionMethods;
use diesel::OptionalExtension;
use diesel::PgConnection;
use diesel::QueryDsl;
use diesel::QueryResult;
use diesel::Queryable;
use diesel::RunQueryDsl;
use rust_decimal::prelude::ToPrimitive;
use std::collections::HashMap;

#[derive(Queryable, Debug, Clone, PartialEq)]
#[diesel(table_name = coordinator_leverages)]
pub(crate) struct CoordinatorLeverage {
    pub id: i32,
    pub trader_leverage: i32,
    pub coordinator_leverage: i32,
}

pub(crate) fn get_all(conn: &mut PgConnection) -> QueryResult<HashMap<u8, u8>> {
    let all = coordinator_leverages::table.load::<CoordinatorLeverage>(conn)?;
    Ok(all
        .iter()
        .map(|lev| {
            (
                lev.trader_leverage.to_u8().expect("to fit"),
                lev.coordinator_leverage.to_u8().expect("to fit"),
            )
        })
        .collect())
}

/// Looks up the coordinator leverage by trader leverage.
///
/// We assume we only have round numbers in the db and hence round `trader_leverage`. If no item can
/// be found we take the max available coordinator_leverage
pub fn find_coordinator_leverage_by_trader_leverage(
    connection: &mut PgConnection,
    trader_lev: f32,
) -> f32 {
    find_by_trader_leverage(connection, &trader_lev).unwrap_or_else(|err| {
        tracing::error!("Failed at loading leverage from db. Falling back to 2.0 {err:#}");
        2.0
    })
}

fn find_by_trader_leverage(connection: &mut PgConnection, trader_lev: &f32) -> Result<f32> {
    let result: Option<CoordinatorLeverage> = coordinator_leverages::table
        .filter(
            coordinator_leverages::trader_leverage
                .eq(&trader_lev.round().to_i32().expect("to fit")),
        )
        .first(connection)
        .optional()?;

    let leverage = match result {
        None => max_coordinator_leverage(connection),
        Some(coordinator_leverages) => coordinator_leverages.coordinator_leverage,
    };

    Ok(leverage.to_f32().expect("to fit"))
}

pub fn max_coordinator_leverage(connection: &mut PgConnection) -> i32 {
    coordinator_leverages::table
        .select(max(coordinator_leverages::coordinator_leverage))
        .first::<Option<i32>>(connection)
        .expect("to be able to execute query")
        .expect("to have at least one leverage in db")
}
