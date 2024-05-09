use crate::schema::reported_errors;
use diesel::prelude::*;
use xxi_node::commons::ReportedError;

#[derive(Insertable, Debug, Clone)]
#[diesel(table_name = reported_errors)]
struct NewReportedError {
    trader_pubkey: String,
    error: String,
    version: String,
}

pub(crate) fn insert(conn: &mut PgConnection, error: ReportedError) -> QueryResult<()> {
    diesel::insert_into(reported_errors::table)
        .values(NewReportedError::from(error))
        .execute(conn)?;

    Ok(())
}

impl From<ReportedError> for NewReportedError {
    fn from(value: ReportedError) -> Self {
        Self {
            trader_pubkey: value.trader_pk.to_string(),
            error: value.msg,
            version: value.version.unwrap_or("<2.3.1".to_owned()),
        }
    }
}
