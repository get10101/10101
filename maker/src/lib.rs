use diesel::PgConnection;
use diesel_migrations::embed_migrations;
use diesel_migrations::EmbeddedMigrations;
use diesel_migrations::MigrationHarness;

#[cfg(test)]
mod tests;

pub mod cli;
pub mod ln;
pub mod logger;
pub mod metrics;
pub mod routes;
pub mod schema;
pub mod trading;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

pub fn run_migration(conn: &mut PgConnection) {
    conn.run_pending_migrations(MIGRATIONS)
        .expect("migrations to succeed");
}
