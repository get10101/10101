use crate::schema;
use crate::schema::transactions;
use anyhow::ensure;
use anyhow::Result;
use bitcoin::Txid;
use diesel::AsChangeset;
use diesel::ExpressionMethods;
use diesel::Insertable;
use diesel::OptionalExtension;
use diesel::PgConnection;
use diesel::QueryDsl;
use diesel::QueryResult;
use diesel::Queryable;
use diesel::QueryableByName;
use diesel::RunQueryDsl;
use std::str::FromStr;
use time::OffsetDateTime;

#[derive(Insertable, QueryableByName, Queryable, Debug, Clone, PartialEq, AsChangeset)]
#[diesel(table_name = transactions)]
pub(crate) struct Transaction {
    pub txid: String,
    pub fee: i64,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

pub(crate) fn get(txid: &str, conn: &mut PgConnection) -> QueryResult<Option<Transaction>> {
    transactions::table
        .filter(transactions::txid.eq(txid))
        .first(conn)
        .optional()
}

pub(crate) fn get_all_without_fees(conn: &mut PgConnection) -> QueryResult<Vec<Transaction>> {
    transactions::table
        .filter(transactions::fee.eq(0))
        .load(conn)
}

pub(crate) fn upsert(tx: Transaction, conn: &mut PgConnection) -> Result<()> {
    let affected_rows = diesel::insert_into(transactions::table)
        .values(tx.clone())
        .on_conflict(schema::transactions::txid)
        .do_update()
        .set(&tx)
        .execute(conn)?;

    ensure!(affected_rows > 0, "Could not upsert transaction");

    Ok(())
}

impl From<ln_dlc_node::transaction::Transaction> for Transaction {
    fn from(value: ln_dlc_node::transaction::Transaction) -> Self {
        Transaction {
            txid: value.txid.to_string(),
            fee: value.fee as i64,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

impl From<Transaction> for ln_dlc_node::transaction::Transaction {
    fn from(value: Transaction) -> Self {
        ln_dlc_node::transaction::Transaction {
            txid: Txid::from_str(&value.txid).expect("valid txid"),
            fee: value.fee as u64,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}
