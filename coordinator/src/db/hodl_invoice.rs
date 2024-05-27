use crate::schema::hodl_invoices;
use anyhow::ensure;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use bitcoin::Amount;
use diesel::ExpressionMethods;
use diesel::PgConnection;
use diesel::RunQueryDsl;
use time::OffsetDateTime;

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
            hodl_invoices::amount_sats.eq(amount_sats as i64),
        ))
        .execute(conn)?;

    ensure!(affected_rows > 0, "Could not insert hodl invoice");

    Ok(())
}

pub fn update_hodl_invoice_pre_image(
    conn: &mut PgConnection,
    hash: &str,
    pre_image: &str,
) -> Result<Amount> {
    let amount: i64 = diesel::update(hodl_invoices::table)
        .filter(hodl_invoices::r_hash.eq(hash))
        .set((
            hodl_invoices::pre_image.eq(pre_image),
            hodl_invoices::updated_at.eq(OffsetDateTime::now_utc()),
        ))
        .returning(hodl_invoices::amount_sats)
        .get_result(conn)?;

    Ok(Amount::from_sat(amount as u64))
}
