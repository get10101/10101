use crate::db;
use crate::node::Node;
use crate::position::models::Position;
use anyhow::Context;
use anyhow::Result;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::PooledConnection;
use diesel::PgConnection;
use time::OffsetDateTime;
use trade::bitmex_client::BitmexClient;
use trade::bitmex_client::Quote;

pub async fn sync(node: Node) -> Result<()> {
    let mut conn = node.pool.get()?;

    let positions = db::positions::Position::get_all_open_or_closing_positions(&mut conn)?;

    // TODO(holzeis): we should not use the bitmex quote here, but rather our own orderbook.
    let current_quote = BitmexClient::get_quote(&node.inner.network, &OffsetDateTime::now_utc())
        .await
        .context("Failed to fetch quote from BitMEX")?;

    for position in positions.iter() {
        if let Err(e) = sync_position(&mut conn, position, current_quote.clone()) {
            tracing::error!(position_id=%position.id, ?current_quote, "Failed to update position's unrealized pnl in database: {e:#}")
        }
    }

    Ok(())
}

fn sync_position(
    conn: &mut PooledConnection<ConnectionManager<PgConnection>>,
    position: &Position,
    quote: Quote,
) -> Result<()> {
    let pnl = position.calculate_coordinator_pnl(quote)?;
    db::positions::Position::update_unrealized_pnl(conn, position.id, pnl)
        .context("Failed to update unrealized pnl in db")?;

    Ok(())
}
