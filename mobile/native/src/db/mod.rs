use crate::db::models::LastLogin;
use anyhow::anyhow;
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

fn connection() -> Result<PooledConnection<ConnectionManager<SqliteConnection>>> {
    let pool = DB.try_get().context("DB uninitialised").cloned()?;

    pool.get()
        .map_err(|e| anyhow!("cannot acquire database connection: {e:#}"))
}

pub fn update_last_login() -> Result<LastLogin> {
    let mut db = connection()?;
    let now = OffsetDateTime::now_utc();
    let last_login = LastLogin::update_last_login(now, &mut db)?;
    Ok(last_login)
}
