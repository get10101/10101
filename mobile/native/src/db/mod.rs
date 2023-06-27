use crate::api;
use crate::db::models::base64_engine;
use crate::db::models::Order;
use crate::db::models::OrderState;
use crate::db::models::PaymentInsertable;
use crate::db::models::PaymentQueryable;
use crate::db::models::Position;
use crate::db::models::SpendableOutputInsertable;
use crate::db::models::SpendableOutputQueryable;
use crate::trade;
use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use base64::Engine;
use bdk::bitcoin;
use diesel::connection::SimpleConnection;
use diesel::r2d2;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::r2d2::PooledConnection;
use diesel::OptionalExtension;
use diesel::SqliteConnection;
use diesel_migrations::embed_migrations;
use diesel_migrations::EmbeddedMigrations;
use diesel_migrations::MigrationHarness;
use state::Storage;
use std::sync::Arc;
use time::Duration;
use time::OffsetDateTime;
use uuid::Uuid;

mod custom_types;
pub mod models;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

/// Sets the number of max connections to the DB.
///
/// We are only allowing 1 connection at a time because given the simplicity of the app currently
/// there is no need for concurrent access to the database.
const MAX_DB_POOL_SIZE: u32 = 1;

static DB: Storage<Arc<Pool<ConnectionManager<SqliteConnection>>>> = Storage::new();

#[derive(Debug)]
pub struct ConnectionOptions {
    pub enable_wal: bool,
    pub enable_foreign_keys: bool,
    pub busy_timeout: Option<Duration>,
}

impl r2d2::CustomizeConnection<SqliteConnection, r2d2::Error> for ConnectionOptions {
    fn on_acquire(&self, conn: &mut SqliteConnection) -> Result<(), r2d2::Error> {
        (|| {
            if let Some(d) = self.busy_timeout {
                conn.batch_execute(&format!(
                    "PRAGMA busy_timeout = {};",
                    d.whole_milliseconds()
                ))?;
            }
            if self.enable_wal {
                conn.batch_execute("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL; PRAGMA wal_autocheckpoint = 1000; PRAGMA wal_checkpoint(TRUNCATE);")?;
            }
            if self.enable_foreign_keys {
                conn.batch_execute("PRAGMA foreign_keys = ON;")?;
            }
            Ok(())
        })()
        .map_err(diesel::r2d2::Error::QueryError)
    }
}

pub fn init_db(db_dir: &str, network: bitcoin::Network) -> Result<()> {
    let database_url = format!("sqlite://{db_dir}/trades-{network}.sqlite");
    let manager = ConnectionManager::<SqliteConnection>::new(database_url);
    let pool = r2d2::Pool::builder()
        .max_size(MAX_DB_POOL_SIZE)
        .connection_customizer(Box::new(ConnectionOptions {
            enable_wal: true,
            enable_foreign_keys: true,
            busy_timeout: Some(Duration::seconds(30)),
        }))
        .build(manager)?;

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

pub fn update_order_state(
    order_id: Uuid,
    order_state: trade::order::OrderState,
) -> Result<trade::order::Order> {
    let mut db = connection()?;

    let order = Order::update_state(order_id.to_string(), order_state.into(), &mut db)
        .context("Failed to update order state")?;

    Ok(order.try_into()?)
}

pub fn get_order(order_id: Uuid) -> Result<trade::order::Order> {
    let mut db = connection()?;
    let order = Order::get(order_id.to_string(), &mut db)?;

    Ok(order.try_into()?)
}

pub fn get_orders_for_ui() -> Result<Vec<trade::order::Order>> {
    let mut db = connection()?;
    let orders = Order::get_without_rejected_and_initial(&mut db)?;

    // TODO: Can probably be optimized with combinator
    let mut mapped = vec![];
    for order in orders {
        mapped.push(order.try_into()?)
    }

    Ok(mapped)
}

pub fn get_filled_orders() -> Result<Vec<trade::order::Order>> {
    let mut db = connection()?;

    let orders = Order::get_by_state(OrderState::Filled, &mut db)?;
    let orders = orders
        .into_iter()
        .map(|order| {
            order
                .try_into()
                .context("Failed to convert to trade::order::Order")
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(orders)
}

/// Returns an order of there is currently an order that is open
pub fn maybe_get_open_orders() -> Result<Vec<trade::order::Order>> {
    let mut db = connection()?;
    let orders = Order::get_by_state(OrderState::Open, &mut db)?;

    let orders = orders
        .into_iter()
        .map(|order| {
            order
                .try_into()
                .context("Failed to convert to trade::order::Order")
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(orders)
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

pub fn insert_position(position: trade::position::Position) -> Result<trade::position::Position> {
    let mut db = connection()?;
    let position = Position::insert(position.into(), &mut db)?;

    Ok(position.into())
}

pub fn get_positions() -> Result<Vec<trade::position::Position>> {
    let mut db = connection()?;
    let positions = Position::get_all(&mut db)?;
    let positions = positions
        .into_iter()
        .map(|position| position.into())
        .collect();

    Ok(positions)
}

pub fn delete_positions() -> Result<()> {
    let mut db = connection()?;
    Position::delete_all(&mut db)?;

    Ok(())
}

pub fn update_position_state(
    contract_symbol: ::trade::ContractSymbol,
    position_state: trade::position::PositionState,
) -> Result<()> {
    let mut db = connection()?;
    Position::update_state(contract_symbol.into(), position_state.into(), &mut db)
        .context("Failed to update position state")?;

    Ok(())
}

pub fn insert_payment(
    payment_hash: lightning::ln::PaymentHash,
    info: ln_dlc_node::PaymentInfo,
) -> Result<()> {
    tracing::info!(?payment_hash, "Inserting payment");

    let mut db = connection()?;

    PaymentInsertable::insert((payment_hash, info).into(), &mut db)?;

    Ok(())
}

pub fn update_payment(
    payment_hash: lightning::ln::PaymentHash,
    htlc_status: ln_dlc_node::HTLCStatus,
    amt_msat: ln_dlc_node::MillisatAmount,
    preimage: Option<lightning::ln::PaymentPreimage>,
    secret: Option<lightning::ln::PaymentSecret>,
) -> Result<()> {
    tracing::info!(?payment_hash, "Updating payment");

    let mut db = connection()?;

    let base64 = base64_engine();

    let preimage = preimage.map(|preimage| base64.encode(preimage.0));
    let secret = secret.map(|secret| base64.encode(secret.0));

    PaymentInsertable::update(
        base64.encode(payment_hash.0),
        htlc_status.into(),
        amt_msat.to_inner().map(|amt| amt as i64),
        preimage,
        secret,
        &mut db,
    )?;

    Ok(())
}

pub fn get_payment(
    payment_hash: lightning::ln::PaymentHash,
) -> Result<Option<(lightning::ln::PaymentHash, ln_dlc_node::PaymentInfo)>> {
    tracing::info!(?payment_hash, "Getting payment");

    let mut db = connection()?;

    let payment =
        PaymentQueryable::get(base64_engine().encode(payment_hash.0), &mut db).optional()?;

    payment.map(|payment| payment.try_into()).transpose()
}

pub fn get_payments() -> Result<Vec<(lightning::ln::PaymentHash, ln_dlc_node::PaymentInfo)>> {
    let mut db = connection()?;
    let payments = PaymentQueryable::get_all(&mut db)?;
    let payments = payments
        .into_iter()
        .map(|payment| payment.try_into())
        .collect::<Result<Vec<_>>>()?;

    let payment_hashes = payments.iter().map(|(a, _)| a).collect::<Vec<_>>();

    tracing::info!(?payment_hashes, "Got all payments");

    Ok(payments)
}

pub fn insert_spendable_output(
    outpoint: lightning::chain::transaction::OutPoint,
    descriptor: lightning::chain::keysinterface::SpendableOutputDescriptor,
) -> Result<()> {
    tracing::info!(?descriptor, "Inserting spendable output");

    let mut db = connection()?;
    SpendableOutputInsertable::insert((outpoint, descriptor).into(), &mut db)?;

    Ok(())
}

pub fn get_spendable_output(
    outpoint: lightning::chain::transaction::OutPoint,
) -> Result<Option<lightning::chain::keysinterface::SpendableOutputDescriptor>> {
    tracing::info!(?outpoint, "Getting spendable output");

    let mut db = connection()?;

    let output = SpendableOutputQueryable::get(outpoint, &mut db).optional()?;

    output.map(|output| output.try_into()).transpose()
}

pub fn get_spendable_outputs(
) -> Result<Vec<lightning::chain::keysinterface::SpendableOutputDescriptor>> {
    let mut db = connection()?;
    let outputs = SpendableOutputQueryable::get_all(&mut db)?;

    let outputs = outputs
        .into_iter()
        .map(|output| output.try_into())
        .collect::<Result<Vec<_>>>()?;

    tracing::info!(?outputs, "Got all spendable outputs");

    Ok(outputs)
}
