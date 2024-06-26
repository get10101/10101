use crate::schema::hodl_invoices;
use crate::schema::sql_types::InvoiceStateType;
use anyhow::ensure;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use bitcoin::Amount;
use diesel::query_builder::QueryId;
use diesel::AsExpression;
use diesel::ExpressionMethods;
use diesel::FromSqlRow;
use diesel::PgConnection;
use diesel::QueryDsl;
use diesel::QueryResult;
use diesel::RunQueryDsl;
use std::any::TypeId;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, FromSqlRow, AsExpression)]
#[diesel(sql_type = InvoiceStateType)]
pub enum InvoiceState {
    Open,
    Accepted,
    Settled,
    Failed,
    Canceled,
}

impl QueryId for InvoiceStateType {
    type QueryId = InvoiceStateType;
    const HAS_STATIC_QUERY_ID: bool = false;

    fn query_id() -> Option<TypeId> {
        None
    }
}

pub fn cancel_pending_hodl_invoices(conn: &mut PgConnection) -> QueryResult<usize> {
    diesel::update(hodl_invoices::table)
        .filter(hodl_invoices::invoice_state.eq_any([InvoiceState::Open, InvoiceState::Accepted]))
        .set(hodl_invoices::invoice_state.eq(InvoiceState::Canceled))
        .execute(conn)
}

pub fn create_hodl_invoice(
    conn: &mut PgConnection,
    r_hash: &str,
    trader_pubkey: PublicKey,
    amount_sats: u64,
) -> Result<()> {
    let affected_rows = diesel::insert_into(hodl_invoices::table)
        .values((
            hodl_invoices::r_hash.eq(r_hash),
            hodl_invoices::trader_pubkey.eq(trader_pubkey.to_string()),
            hodl_invoices::invoice_state.eq(InvoiceState::Open),
            hodl_invoices::amount_sats.eq(amount_sats as i64),
        ))
        .execute(conn)?;

    ensure!(affected_rows > 0, "Could not insert hodl invoice");

    Ok(())
}

pub fn get_r_hash_by_order_id(conn: &mut PgConnection, order_id: Uuid) -> QueryResult<String> {
    hodl_invoices::table
        .filter(hodl_invoices::order_id.eq(order_id))
        .select(hodl_invoices::r_hash)
        .get_result(conn)
}

/// Returns the pre image of the hodl invoice associated with the order id
/// If the hodl invoice can not be found a [`Not Found`] error is returned
/// If the hodl invoice is found the pre_image is optional, as it might have not yet been set.
pub fn get_pre_image_by_order_id(
    conn: &mut PgConnection,
    order_id: Uuid,
) -> QueryResult<Option<String>> {
    hodl_invoices::table
        .filter(hodl_invoices::order_id.eq(order_id))
        .select(hodl_invoices::pre_image)
        .get_result(conn)
}

pub fn update_hodl_invoice_to_accepted(
    conn: &mut PgConnection,
    hash: &str,
    pre_image: &str,
    order_id: Uuid,
) -> Result<Amount> {
    let amount: i64 = diesel::update(hodl_invoices::table)
        .filter(hodl_invoices::r_hash.eq(hash))
        .set((
            hodl_invoices::pre_image.eq(pre_image),
            hodl_invoices::updated_at.eq(OffsetDateTime::now_utc()),
            hodl_invoices::invoice_state.eq(InvoiceState::Accepted),
            hodl_invoices::order_id.eq(order_id),
        ))
        .returning(hodl_invoices::amount_sats)
        .get_result(conn)?;

    Ok(Amount::from_sat(amount as u64))
}

pub fn update_hodl_invoice_to_settled(
    conn: &mut PgConnection,
    r_hash: String,
) -> QueryResult<Option<String>> {
    diesel::update(hodl_invoices::table)
        .filter(hodl_invoices::r_hash.eq(r_hash))
        .set((
            hodl_invoices::updated_at.eq(OffsetDateTime::now_utc()),
            hodl_invoices::invoice_state.eq(InvoiceState::Settled),
        ))
        .returning(hodl_invoices::pre_image)
        .get_result(conn)
}

pub fn update_hodl_invoice_to_canceled(
    conn: &mut PgConnection,
    r_hash: String,
) -> QueryResult<usize> {
    diesel::update(hodl_invoices::table)
        .filter(hodl_invoices::r_hash.eq(r_hash))
        .set((
            hodl_invoices::updated_at.eq(OffsetDateTime::now_utc()),
            hodl_invoices::invoice_state.eq(InvoiceState::Canceled),
        ))
        .execute(conn)
}
