use crate::db;
use crate::db::user::User;
use anyhow::ensure;
use anyhow::Context;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use diesel::PgConnection;

pub fn check_version(conn: &mut PgConnection, trader_id: &PublicKey) -> Result<()> {
    let user: User = db::user::get_user(conn, trader_id)?.context("Couldn't find user")?;

    let app_version = user.version.context("No version found")?;

    let coordinator_version = env!("CARGO_PKG_VERSION").to_string();
    ensure!(
        app_version == coordinator_version,
        format!("Please upgrade to the latest version: {coordinator_version}")
    );

    Ok(())
}
