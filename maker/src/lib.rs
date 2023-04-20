pub mod cli;
pub mod logger;
pub mod routes;
pub mod schema;
pub mod trading;

#[cfg(test)]
mod tests;

use diesel::PgConnection;
use diesel_migrations::embed_migrations;
use diesel_migrations::EmbeddedMigrations;
use diesel_migrations::MigrationHarness;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

pub fn run_migration(conn: &mut PgConnection) {
    conn.run_pending_migrations(MIGRATIONS)
        .expect("migrations to succeed");
}
