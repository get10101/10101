use crate::funding_fee;
use crate::schema::funding_rates;
use crate::to_nearest_hour_in_the_past;
use anyhow::Context;
use anyhow::Result;
use diesel::prelude::*;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use time::OffsetDateTime;

#[derive(Insertable, Debug)]
#[diesel(table_name = funding_rates)]
struct NewFundingRate {
    start_date: OffsetDateTime,
    end_date: OffsetDateTime,
    rate: f32,
}

#[derive(Queryable, Debug)]
struct FundingRate {
    #[diesel(column_name = "id")]
    _id: i32,
    start_date: OffsetDateTime,
    end_date: OffsetDateTime,
    rate: f32,
    #[diesel(column_name = "timestamp")]
    _timestamp: OffsetDateTime,
}

pub(crate) fn insert(
    conn: &mut PgConnection,
    funding_rates: &[funding_fee::FundingRate],
) -> Result<()> {
    let funding_rates = funding_rates
        .iter()
        .copied()
        .map(NewFundingRate::from)
        .collect::<Vec<_>>();

        Ok(())
    })
}

fn insert_one(conn: &mut PgConnection, params: &funding_fee::FundingRate) -> QueryResult<()> {
    let affected_rows = diesel::insert_into(funding_rates::table)
        .values(funding_rates)
        .execute(conn)?;

    if affected_rows == 0 {
        bail!("Failed to insert funding rates");
    }

    Ok(())
}

/// Get the funding rate with an end date that is equal to the current date to the nearest hour.
pub(crate) fn get_funding_rate_charged_in_the_last_hour(
    conn: &mut PgConnection,
) -> QueryResult<Option<funding_fee::FundingRate>> {
    let now = OffsetDateTime::now_utc();
    let now = to_nearest_hour_in_the_past(now);

    let funding_rate: Option<FundingRate> = funding_rates::table
        .filter(funding_rates::end_date.eq(now))
        .first::<FundingRate>(conn)
        .optional()?;

    Ok(funding_rate.map(funding_fee::FundingRate::from))
}

impl From<FundingRate> for funding_fee::FundingRate {
    fn from(value: FundingRate) -> Self {
        Self::new(
            Decimal::from_f32(value.rate).expect("to fit"),
            value.start_date,
            value.end_date,
        )
    }
}

impl From<xxi_node::commons::FundingRate> for NewFundingRate {
    fn from(value: xxi_node::commons::FundingRate) -> Self {
        Self {
            start_date: value.start_date(),
            end_date: value.end_date(),
            rate: value.rate().to_f32().expect("to fit"),
        }
    }
}
