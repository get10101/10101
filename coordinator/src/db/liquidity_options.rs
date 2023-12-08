use crate::schema::liquidity_options;
use diesel::ExpressionMethods;
use diesel::PgConnection;
use diesel::QueryDsl;
use diesel::QueryResult;
use diesel::Queryable;
use diesel::RunQueryDsl;
use time::OffsetDateTime;

#[derive(Queryable, Debug, Clone, PartialEq)]
#[diesel(table_name = liquidity_options)]
pub(crate) struct LiquidityOption {
    pub id: i32,
    pub rank: i16,
    pub title: String,
    /// amount the user can trade up to in sats
    pub trade_up_to_sats: i64,
    /// min deposit in sats
    pub min_deposit_sats: i64,
    /// max deposit in sats
    pub max_deposit_sats: i64,
    /// min fee in sats
    pub min_fee_sats: Option<i64>,
    pub fee_percentage: f64,
    pub coordinator_leverage: f32,
    pub active: bool,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

/// returns all active liquidity options
pub(crate) fn get_active(conn: &mut PgConnection) -> QueryResult<Vec<commons::LiquidityOption>> {
    let options = liquidity_options::table
        .filter(liquidity_options::active.eq(true))
        .load::<LiquidityOption>(conn)?;

    let options = options
        .into_iter()
        .map(commons::LiquidityOption::from)
        .collect();
    Ok(options)
}

pub(crate) fn get(
    conn: &mut PgConnection,
    liquidity_option_id: i32,
) -> QueryResult<commons::LiquidityOption> {
    let option: LiquidityOption = liquidity_options::table
        .filter(liquidity_options::id.eq(liquidity_option_id))
        .get_result(conn)?;
    Ok(option.into())
}

impl From<LiquidityOption> for commons::LiquidityOption {
    fn from(value: LiquidityOption) -> Self {
        commons::LiquidityOption {
            id: value.id,
            rank: value.rank as usize,
            title: value.title,
            trade_up_to_sats: value.trade_up_to_sats as u64,
            min_deposit_sats: value.min_deposit_sats as u64,
            max_deposit_sats: value.max_deposit_sats as u64,
            min_fee_sats: value.min_fee_sats.unwrap_or(0) as u64,
            fee_percentage: value.fee_percentage,
            coordinator_leverage: value.coordinator_leverage,
            created_at: value.created_at,
            updated_at: value.updated_at,
            active: value.active,
        }
    }
}
