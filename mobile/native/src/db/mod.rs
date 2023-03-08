use crate::api;
use crate::db::models::Order;
use crate::db::models::OrderState;
use crate::trade;
use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use diesel::r2d2;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::r2d2::PooledConnection;
use diesel::SqliteConnection;
use diesel_migrations::embed_migrations;
use diesel_migrations::EmbeddedMigrations;
use diesel_migrations::MigrationHarness;
use state::Storage;
use std::sync::Arc;
use time::OffsetDateTime;
use uuid::Uuid;

mod custom_types;
pub mod models;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

static DB: Storage<Arc<Pool<ConnectionManager<SqliteConnection>>>> = Storage::new();

pub fn init_db(db_dir: String) -> Result<()> {
    let database_url = format!("sqlite://{db_dir}/trader.sqlite");
    let manager = ConnectionManager::<SqliteConnection>::new(database_url);
    let pool = r2d2::Pool::builder().build(manager)?;

    let mut connection = pool.get()?;

    connection
        .run_pending_migrations(MIGRATIONS)
        .map_err(|e| anyhow!("could not run db migration: {e:#}"))?;
    tracing::debug!("Database migration run - db initialized");

    DB.set(Arc::new(pool));

    Ok(())
}

pub fn connection() -> Result<PooledConnection<ConnectionManager<SqliteConnection>>> {
    let pool = DB.try_get().context("DB uninitialised").cloned()?;

    pool.get()
        .map_err(|e| anyhow!("cannot acquire database connection: {e:#}"))
}

pub fn update_last_login() -> Result<api::LastLogin> {
    let mut db = connection()?;
    let now = OffsetDateTime::now_utc();
    let last_login = models::LastLogin::update_last_login(now, &mut db)?.into();
    Ok(last_login)
}

pub fn insert_order(order: trade::order::Order) -> Result<trade::order::Order> {
    let mut db = connection()?;
    let order = Order::insert(order.into(), &mut db)?;

    Ok(order.try_into()?)
}

pub fn update_order_state(order_id: Uuid, order_state: trade::order::OrderState) -> Result<()> {
    let mut db = connection()?;
    Order::update_state(order_id.to_string(), order_state.into(), &mut db)
        .context("Failed to update order state")?;

    Ok(())
}

pub fn get_order(order_id: Uuid) -> Result<trade::order::Order> {
    let mut db = connection()?;
    let order = Order::get(order_id.to_string(), &mut db)?;

    Ok(order.try_into()?)
}

/// Returns an order of there is currently an order that is being filled
pub fn maybe_get_order_in_filling() -> Result<Option<trade::order::Order>> {
    let mut db = connection()?;
    let orders = Order::get_by_state(OrderState::Filling, &mut db)?;

    if orders.is_empty() {
        return Ok(None);
    }

    if orders.len() > 1 {
        bail!("More than one order is being filled at the same time, this should not happen.")
    }

    let first = orders
        .get(0)
        .expect("at this point we know there is exactly one order");

    Ok(Some(first.clone().try_into()?))
}

pub fn delete_order(order_id: Uuid) -> Result<()> {
    let mut db = connection()?;
    Order::delete(order_id.to_string(), &mut db)?;

    Ok(())
}
