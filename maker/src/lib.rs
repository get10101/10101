use diesel::PgConnection;
use diesel_migrations::embed_migrations;
use diesel_migrations::EmbeddedMigrations;
use diesel_migrations::MigrationHarness;
use ln_dlc_node::node::GossipSourceConfig;
use ln_dlc_node::node::LnDlcNodeSettings;
use std::time::Duration;

#[cfg(test)]
mod tests;

pub mod cli;
pub mod health;
pub mod ln;
pub mod logger;
pub mod metrics;
pub mod orderbook_ws;
pub mod position;
pub mod routes;
pub mod schema;
pub mod storage;
pub mod trading;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

pub fn run_migration(conn: &mut PgConnection) {
    conn.run_pending_migrations(MIGRATIONS)
        .expect("migrations to succeed");
}

pub fn ln_dlc_node_settings(rgs_server_url: Option<String>) -> LnDlcNodeSettings {
    let gossip_source_config = match rgs_server_url {
        Some(server_url) => GossipSourceConfig::RapidGossipSync { server_url },
        None => GossipSourceConfig::P2pNetwork,
    };

    LnDlcNodeSettings {
        off_chain_sync_interval: Duration::from_secs(5),
        on_chain_sync_interval: Duration::from_secs(300),
        fee_rate_sync_interval: Duration::from_secs(20),
        dlc_manager_periodic_check_interval: Duration::from_secs(30),
        sub_channel_manager_periodic_check_interval: Duration::from_secs(30),
        shadow_sync_interval: Duration::from_secs(600),
        forwarding_fee_proportional_millionths: 50,
        bdk_client_stop_gap: 20,
        bdk_client_concurrency: 4,
        gossip_source_config,
    }
}
