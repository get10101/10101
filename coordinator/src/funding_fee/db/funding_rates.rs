use crate::schema::funding_rates;
use anyhow::bail;
use anyhow::Result;
use diesel::prelude::*;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use time::OffsetDateTime;
use xxi_node::commons::to_nearest_hour_in_the_past;

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

pub fn insert_funding_rates(
    conn: &mut PgConnection,
    funding_rates: &[xxi_node::commons::FundingRate],
) -> Result<()> {
    let funding_rates = funding_rates
        .iter()
        .copied()
        .map(NewFundingRate::from)
        .collect::<Vec<_>>();

    let affected_rows = diesel::insert_into(funding_rates::table)
        .values(funding_rates)
        .execute(conn)?;

    if affected_rows == 0 {
        bail!("Failed to insert funding rates");
    }

    Ok(())
}

pub fn get_next_funding_rate(
    conn: &mut PgConnection,
) -> QueryResult<Option<xxi_node::commons::FundingRate>> {
    let funding_rate: Option<FundingRate> = funding_rates::table
        .order(funding_rates::end_date.desc())
        .first::<FundingRate>(conn)
        .optional()?;

    let funding_rate = funding_rate.map(xxi_node::commons::FundingRate::from);

    Ok(funding_rate)
}

/// Get the funding rate with an end date that is equal to the current date to the nearest hour.
pub fn get_funding_rate_charged_in_the_last_hour(
    conn: &mut PgConnection,
) -> QueryResult<Option<xxi_node::commons::FundingRate>> {
    let now = OffsetDateTime::now_utc();
    let now = to_nearest_hour_in_the_past(now);

    let funding_rate: Option<FundingRate> = funding_rates::table
        .filter(funding_rates::end_date.eq(now))
        .first::<FundingRate>(conn)
        .optional()?;

    Ok(funding_rate.map(xxi_node::commons::FundingRate::from))
}

impl From<FundingRate> for xxi_node::commons::FundingRate {
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
