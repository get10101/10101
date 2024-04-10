use crate::config;
use crate::db::models::FailureReason;
use crate::db::models::NewTrade;
use crate::db::models::Order;
use crate::db::models::OrderState;
use crate::db::models::Position;
use crate::db::models::SpendableOutputInsertable;
use crate::db::models::SpendableOutputQueryable;
use crate::db::models::Trade;
use crate::db::models::Transaction;
use crate::trade;
use anyhow::anyhow;
use anyhow::Context;
use anyhow::Result;
use bitcoin::Amount;
use bitcoin::Network;
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
use parking_lot::Mutex;
use rusqlite::backup::Backup;
use rusqlite::Connection;
use rusqlite::OpenFlags;
use state::Storage;
use std::path::Path;
use std::sync::Arc;
use time::Duration;
use time::OffsetDateTime;
use uuid::Uuid;

mod custom_types;
pub mod dlc_messages;
pub mod last_outbound_dlc_messages;
pub mod models;
pub mod polls;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

/// Sets the number of max connections to the DB.
///
/// We are only allowing 1 connection at a time because given the simplicity of the app currently
/// there is no need for concurrent access to the database.
const MAX_DB_POOL_SIZE: u32 = 1;

static DB: Storage<Arc<Pool<ConnectionManager<SqliteConnection>>>> = Storage::new();
static BACKUP_CONNECTION: Storage<Arc<Mutex<Connection>>> = Storage::new();

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

pub fn init_db(db_dir: &str, network: Network) -> Result<()> {
    if DB.try_get().is_some() {
        return Ok(());
    }

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

    tracing::debug!("Opening read-only backup connection");
    let backup_conn = Connection::open_with_flags(
        format!("{db_dir}/trades-{network}.sqlite"),
        // [`OpenFlags::SQLITE_OPEN_READ_ONLY`]: The database is opened in read-only mode. If the database does not already exist, an error is returned
        // [`OpenFlags::SQLITE_OPEN_NO_MUTEX`]: The new database connection will use the "multi-thread" threading mode. This means that separate threads are allowed to use SQLite at the same time, as long as each thread is using a different database connection.
        // https://www.sqlite.org/c3ref/open.html
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    BACKUP_CONNECTION.set(Arc::new(Mutex::new(backup_conn)));

    Ok(())
}

/// Creates a backup of the database
///
/// Returns the path to the file of the database backup
pub fn back_up() -> Result<String> {
    let connection = BACKUP_CONNECTION.get().lock();
    let backup_dir = config::get_backup_dir();
    let dst_path = Path::new(&backup_dir).join("trades.sqlite");

    let mut dst = Connection::open(dst_path.clone())?;
    let backup = Backup::new(&connection, &mut dst)?;

    backup.run_to_completion(100, std::time::Duration::from_millis(250), None)?;

    Ok(dst_path.to_string_lossy().to_string())
}

pub fn connection() -> Result<PooledConnection<ConnectionManager<SqliteConnection>>> {
    let pool = DB.try_get().context("DB uninitialised").cloned()?;

    pool.get()
        .map_err(|e| anyhow!("cannot acquire database connection: {e:#}"))
}

pub fn insert_order(order: trade::order::Order) -> Result<trade::order::Order> {
    let mut db = connection()?;
    let order = Order::insert(order.into(), &mut db)?;

    Ok(order.try_into()?)
}

impl From<trade::order::OrderState> for OrderState {
    fn from(value: trade::order::OrderState) -> Self {
        match value {
            trade::order::OrderState::Initial => OrderState::Initial,
            trade::order::OrderState::Rejected => OrderState::Rejected,
            trade::order::OrderState::Open => OrderState::Open,
            trade::order::OrderState::Filling { .. } => OrderState::Filling,
            trade::order::OrderState::Failed { .. } => OrderState::Failed,
            trade::order::OrderState::Filled { .. } => OrderState::Filled,
        }
    }
}

pub fn set_order_state_to_failed(
    order_id: Uuid,
    failure_reason: FailureReason,
    execution_price: Option<f32>,
) -> Result<trade::order::Order> {
    let mut db = connection()?;
    let order = Order::set_order_state_to_failed(
        order_id.to_string(),
        execution_price,
        None,
        failure_reason,
        &mut db,
    )
    .context("Failed to set order state to failed")?;

    Ok(order.try_into()?)
}

pub fn set_order_state_to_open(order_id: Uuid) -> Result<trade::order::Order> {
    let mut db = connection()?;
    let order = Order::set_order_state_to_open(order_id.to_string(), &mut db)
        .context("Failed to set order state to open")?;

    Ok(order.try_into()?)
}

pub fn get_order(order_id: Uuid) -> Result<Option<trade::order::Order>> {
    let mut db = connection()?;
    let order = Order::get(order_id.to_string(), &mut db)?;

    let order = match order {
        Some(order) => Some(trade::order::Order::try_from(order)?),
        None => None,
    };

    Ok(order)
}

pub fn get_orders_for_ui() -> Result<Vec<trade::order::Order>> {
    let mut db = connection()?;
    let orders = Order::get_without_rejected_and_initial(&mut db)?;

    Ok(orders
        .into_iter()
        .map(TryInto::try_into)
        .collect::<Result<_, _>>()?)
}

pub fn get_async_order() -> Result<Option<trade::order::Order>> {
    let mut db = connection()?;
    let order = Order::get_async_order(&mut db)?;

    let order: Option<trade::order::Order> = match order {
        Some(order) => {
            let order = order.try_into()?;
            Some(order)
        }
        None => None,
    };

    Ok(order)
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

/// Returns all open orders
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

pub fn get_last_failed_order() -> Result<Option<trade::order::Order>> {
    let mut db = connection()?;

    let mut orders = Order::get_by_state(OrderState::Failed, &mut db)?;

    orders.sort_by(|a, b| b.creation_timestamp.cmp(&a.creation_timestamp));

    let order = match orders.first() {
        Some(order) => Some(order.clone().try_into()?),
        None => None,
    };

    Ok(order)
}

pub fn set_order_state_to_filled(
    order_id: Uuid,
    execution_price: f32,
    matching_fee: Amount,
) -> Result<trade::order::Order> {
    let mut connection = connection()?;
    let order =
        Order::set_order_state_to_filled(order_id, execution_price, matching_fee, &mut connection)?;
    Ok(order.try_into()?)
}

pub fn set_order_state_to_filling(
    order_id: Uuid,
    execution_price: f32,
    matching_fee: Amount,
) -> Result<trade::order::Order> {
    let mut connection = connection()?;
    let order = Order::set_order_state_to_filling(
        order_id,
        execution_price,
        matching_fee,
        &mut connection,
    )?;
    Ok(order.try_into()?)
}

/// Return an [`Order`] that is currently in [`OrderState::Filling`].
pub fn get_order_in_filling() -> Result<Option<trade::order::Order>> {
    let mut db = connection()?;

    let mut orders = Order::get_by_state(OrderState::Filling, &mut db)?;

    orders.sort_by(|a, b| b.creation_timestamp.cmp(&a.creation_timestamp));

    let order = match orders.as_slice() {
        [] => return Ok(None),
        [order] => order,
        // We strive to only have one order at a time in `OrderState::Filling`. But, if we do not
        // manage, we take the most oldest one.
        [oldest_order, rest @ ..] => {
            tracing::warn!(
                id = %oldest_order.id,
                "Found more than one order in filling. Using oldest one",
            );

            // Clean up other orders in `OrderState::Filling`.
            for order in rest {
                tracing::debug!(
                    id = %order.id,
                    "Setting unexpected Filling order to Failed"
                );

                if let Err(e) = Order::set_order_state_to_failed(
                    order.id.clone(),
                    order.execution_price,
                    None,
                    FailureReason::TimedOut,
                    &mut db,
                ) {
                    tracing::error!("Failed to set old Filling order to Failed: {e:#}");
                };
            }

            oldest_order
        }
    };

    Ok(Some(order.clone().try_into()?))
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
) -> Result<trade::position::Position> {
    let mut db = connection()?;
    let position = Position::update_state(contract_symbol.into(), position_state.into(), &mut db)
        .context("Failed to update position state")?;

    Ok(position.into())
}

pub fn update_position(resized_position: trade::position::Position) -> Result<()> {
    let mut db = connection()?;
    Position::update_position(&mut db, resized_position.into())
        .context("Failed to update position state")?;

    Ok(())
}

pub fn rollover_position(
    contract_symbol: ::trade::ContractSymbol,
    expiry_timestamp: OffsetDateTime,
) -> Result<()> {
    let mut db = connection()?;
    Position::rollover(&mut db, contract_symbol.into(), expiry_timestamp)
        .context("Failed to rollover position")?;

    Ok(())
}

pub fn insert_spendable_output(
    outpoint: lightning::chain::transaction::OutPoint,
    descriptor: lightning::sign::SpendableOutputDescriptor,
) -> Result<()> {
    tracing::debug!(?descriptor, "Inserting spendable output");

    let mut db = connection()?;
    SpendableOutputInsertable::insert((outpoint, descriptor).into(), &mut db)?;

    Ok(())
}

pub fn get_spendable_output(
    outpoint: lightning::chain::transaction::OutPoint,
) -> Result<Option<lightning::sign::SpendableOutputDescriptor>> {
    tracing::debug!(?outpoint, "Getting spendable output");

    let mut db = connection()?;

    let output = SpendableOutputQueryable::get(outpoint, &mut db).optional()?;

    output.map(|output| output.try_into()).transpose()
}

pub fn delete_spendable_output(outpoint: lightning::chain::transaction::OutPoint) -> Result<()> {
    tracing::debug!(?outpoint, "Removing spendable output");

    let mut db = connection()?;
    SpendableOutputQueryable::delete(outpoint, &mut db)?;

    Ok(())
}

pub fn get_spendable_outputs() -> Result<Vec<lightning::sign::SpendableOutputDescriptor>> {
    let mut db = connection()?;
    let outputs = SpendableOutputQueryable::get_all(&mut db)?;

    let outputs = outputs
        .into_iter()
        .map(|output| output.try_into())
        .collect::<Result<Vec<_>>>()?;

    tracing::debug!(?outputs, "Got all spendable outputs");

    Ok(outputs)
}

// Transaction

pub fn upsert_transaction(transaction: ln_dlc_node::transaction::Transaction) -> Result<()> {
    tracing::debug!(?transaction, "Upserting transaction");
    let mut db = connection()?;
    Transaction::upsert(transaction.into(), &mut db)
}

pub fn get_transaction(txid: &str) -> Result<Option<ln_dlc_node::transaction::Transaction>> {
    tracing::debug!(%txid, "Getting transaction");
    let mut db = connection()?;
    let transaction = Transaction::get(txid, &mut db)
        .map_err(|e| anyhow!("{e:#}"))?
        .map(|t| t.into());

    Ok(transaction)
}

pub fn get_all_transactions_without_fees() -> Result<Vec<ln_dlc_node::transaction::Transaction>> {
    let mut db = connection()?;
    let transactions = Transaction::get_all_without_fees(&mut db)?
        .into_iter()
        .map(|t| t.into())
        .collect::<Vec<_>>();

    tracing::debug!(?transactions, "Got all transactions");

    Ok(transactions)
}

pub fn get_all_trades() -> Result<Vec<crate::trade::Trade>> {
    let mut db = connection()?;

    let trades = Trade::get_all(&mut db)?;
    let trades = trades
        .into_iter()
        .map(|trade| trade.into())
        .collect::<Vec<_>>();

    Ok(trades)
}

pub fn insert_trade(trade: crate::trade::Trade) -> Result<()> {
    let mut db = connection()?;

    NewTrade::insert(&mut db, trade.into())?;

    Ok(())
}

/// Returns a list of polls which have been answered or should be ignored
pub fn load_ignored_or_answered_polls() -> Result<Vec<polls::AnsweredOrIgnored>> {
    let mut db = connection()?;
    let answered_polls = polls::get(&mut db)?;
    for i in &answered_polls {
        tracing::debug!(id = i.poll_id, "Ignored poll")
    }
    Ok(answered_polls)
}

/// A poll inserted into this table was either answered or should be ignored in the future.
pub fn set_poll_to_ignored_or_answered(poll_id: i32) -> Result<()> {
    let mut db = connection()?;
    polls::insert(&mut db, poll_id)?;
    Ok(())
}
pub fn delete_answered_poll_cache() -> Result<()> {
    let mut db = connection()?;
    polls::delete_all(&mut db)?;
    Ok(())
}
