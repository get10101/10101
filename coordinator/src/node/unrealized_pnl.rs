use crate::db;
use crate::node::Node;
use crate::position::models::Position;
use anyhow::Context;
use anyhow::Result;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::PooledConnection;
use diesel::PgConnection;
use rust_decimal::Decimal;
use time::OffsetDateTime;
use trade::bitmex_client::BitmexClient;
use trade::bitmex_client::Quote;
use trade::cfd::calculate_pnl;
use trade::Direction;

pub async fn sync(node: Node) -> Result<()> {
    let mut conn = node.pool.get()?;

    let positions = db::positions::Position::get_all_open_or_closing_positions(&mut conn)?;
    let current_quote = BitmexClient::get_quote(&OffsetDateTime::now_utc())
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
    let closing_price = match position.closing_price {
        None => match position.direction {
            trade::Direction::Long => quote.bid_price,
            trade::Direction::Short => quote.ask_price,
        },
        Some(closing_price) => {
            Decimal::try_from(closing_price).expect("f32 closing price to fit into decimal")
        }
    };

    let average_entry_price = Decimal::try_from(position.average_entry_price)
        .context("Failed to convert average entry price to Decimal")?;

    let (long_leverage, short_leverage) = match position.direction {
        Direction::Long => (position.leverage, 1.0_f32),
        Direction::Short => (1.0_f32, position.leverage),
    };

    // the position in the database is the trader's position, our direction is opposite
    let direction = position.direction.opposite();

    let pnl = calculate_pnl(
        average_entry_price,
        closing_price,
        position.quantity,
        long_leverage,
        short_leverage,
        direction,
    )
    .context("Failed to calculate pnl for position")?;

    db::positions::Position::update_unrealized_pnl(conn, position.id, pnl)
        .context("Failed to update unrealized pnl in db")?;

    Ok(())
}
