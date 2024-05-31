use crate::schema::metrics;
use anyhow::ensure;
use anyhow::Result;
use diesel::ExpressionMethods;
use diesel::PgConnection;
use diesel::RunQueryDsl;

pub fn create_metrics_entry(conn: &mut PgConnection, on_chain_balance: u64) -> Result<()> {
    let affected_rows = diesel::insert_into(metrics::table)
        .values(metrics::on_chain_balance_sats.eq(on_chain_balance as i64))
        .execute(conn)?;

    ensure!(affected_rows > 0, "Could not insert metric entry");

    Ok(())
}
