use crate::schema;
use crate::schema::answered_polls;
use anyhow::ensure;
use anyhow::Result;
use diesel::Insertable;
use diesel::QueryResult;
use diesel::Queryable;
use diesel::QueryableByName;
use diesel::RunQueryDsl;
use diesel::SqliteConnection;
use time::OffsetDateTime;

#[derive(Insertable, Debug, Clone, PartialEq)]
#[diesel(table_name = answered_polls)]
pub struct NewAnsweredOrIgnored {
    pub poll_id: i32,
    pub timestamp: i64,
}

#[derive(QueryableByName, Queryable, Debug, Clone, PartialEq)]
#[diesel(table_name = answered_polls)]
pub struct AnsweredOrIgnored {
    pub id: i32,
    pub poll_id: i32,
    pub timestamp: i64,
}

pub(crate) fn get(conn: &mut SqliteConnection) -> QueryResult<Vec<AnsweredOrIgnored>> {
    let result = schema::answered_polls::table.load(conn)?;
    Ok(result)
}

pub(crate) fn insert(conn: &mut SqliteConnection, poll_id: i32) -> Result<()> {
    let affected_rows = diesel::insert_into(schema::answered_polls::table)
        .values(NewAnsweredOrIgnored {
            poll_id,
            timestamp: OffsetDateTime::now_utc().unix_timestamp(),
        })
        .execute(conn)?;

    ensure!(affected_rows > 0, "Could not insert answered poll");

    Ok(())
}

pub(crate) fn delete_all(conn: &mut SqliteConnection) -> Result<()> {
    diesel::delete(schema::answered_polls::table).execute(conn)?;

    Ok(())
}
