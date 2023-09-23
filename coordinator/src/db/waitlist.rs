use crate::schema::waitlist;
use diesel::prelude::*;
use serde::Serialize;
use time::OffsetDateTime;

#[derive(Queryable, Insertable, Identifiable, Debug, PartialEq, Clone, Serialize)]
#[diesel(table_name = waitlist)]
#[diesel(primary_key(email))]
pub struct WaitlistEntry {
    pub email: String,
    /// Timestamp when the user was added to the waitlist
    pub created_timestamp: OffsetDateTime,
    /// Is the user allowed to access the app
    pub allowed: bool,
    /// Timestamp when the user was allowed access
    pub allowed_timestamp: Option<OffsetDateTime>,
}

impl WaitlistEntry {
    pub fn new(email: String, allowed: bool) -> Self {
        let allowed_timestamp = if allowed {
            Some(OffsetDateTime::now_utc())
        } else {
            None
        };

        Self {
            email,
            created_timestamp: OffsetDateTime::now_utc(),
            allowed,
            allowed_timestamp,
        }
    }

    pub fn allow(&mut self) {
        self.allowed = true;
        self.allowed_timestamp = Some(OffsetDateTime::now_utc());
    }
}

/// Insert or modify a waitlist entry
pub fn upsert(conn: &mut PgConnection, entry: WaitlistEntry) -> QueryResult<WaitlistEntry> {
    diesel::insert_into(waitlist::table)
        .values(entry.clone())
        .on_conflict(waitlist::email)
        .do_update()
        .set((
            waitlist::allowed.eq(entry.allowed),
            waitlist::allowed_timestamp.eq(entry.allowed_timestamp),
        ))
        .get_result(conn)
}

pub fn with_email(conn: &mut PgConnection, email: &str) -> QueryResult<Option<WaitlistEntry>> {
    Ok(waitlist::table
        .filter(waitlist::email.eq(email))
        .load::<WaitlistEntry>(conn)?
        .into_iter()
        .next())
}

pub fn all(conn: &mut PgConnection) -> QueryResult<Vec<WaitlistEntry>> {
    waitlist::table.load::<WaitlistEntry>(conn)
}
