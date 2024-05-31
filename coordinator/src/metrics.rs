use crate::db;
use crate::node::Node;
use anyhow::Result;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::PooledConnection;
use diesel::PgConnection;

pub fn collect_metrics(
    mut conn: PooledConnection<ConnectionManager<PgConnection>>,
    node: Node,
) -> Result<()> {
    let balance = node.inner.wallet().get_balance();
    db::metrics::create_metrics_entry(
        &mut conn,
        balance.confirmed + balance.untrusted_pending + balance.trusted_pending + balance.immature,
    )?;
    // TODO: also collect LN balance

    Ok(())
}
