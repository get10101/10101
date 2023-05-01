use coordinator_commons::RegisterParams;
use diesel::prelude::*;
use serde::Deserialize;
use serde::Serialize;
use time::OffsetDateTime;

use crate::schema::users;

#[derive(Insertable, Queryable, Identifiable, Debug, Clone, Serialize, Deserialize)]
#[diesel(primary_key(id))]
pub struct User {
    #[diesel(deserialize_as = i32)]
    pub id: Option<i32>,
    pub pubkey: String,
    pub email: String,
    pub nostr: String,
    pub timestamp: OffsetDateTime,
}

impl From<RegisterParams> for User {
    fn from(value: RegisterParams) -> Self {
        User {
            id: None,
            pubkey: value.pubkey.to_string(),
            email: value.email.unwrap_or("".to_owned()),
            nostr: value.nostr.unwrap_or("".to_owned()),
            timestamp: OffsetDateTime::now_utc(),
        }
    }
}

pub fn all(conn: &mut PgConnection) -> QueryResult<Vec<User>> {
    users::dsl::users.load(conn)
}

pub fn insert(conn: &mut PgConnection, user: User) -> QueryResult<User> {
    let user: User = diesel::insert_into(users::table)
        .values(&user)
        .get_result(conn)?;
    Ok(user)
}
