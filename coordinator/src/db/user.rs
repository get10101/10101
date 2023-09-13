use crate::schema::users;
use anyhow::bail;
use anyhow::Result;
use coordinator_commons::RegisterParams;
use coordinator_commons::TokenUpdateParams;
use diesel::prelude::*;
use serde::Deserialize;
use serde::Serialize;
use time::OffsetDateTime;

#[derive(Insertable, Queryable, Identifiable, Debug, Clone, Serialize, Deserialize)]
#[diesel(primary_key(id))]
pub struct User {
    #[diesel(deserialize_as = i32)]
    pub id: Option<i32>,
    pub pubkey: String,
    pub email: String,
    pub nostr: String,
    pub timestamp: OffsetDateTime,
    pub fcm_token: String,
}

impl From<RegisterParams> for User {
    fn from(value: RegisterParams) -> Self {
        User {
            id: None,
            pubkey: value.pubkey.to_string(),
            email: value.email.unwrap_or("".to_owned()),
            nostr: value.nostr.unwrap_or("".to_owned()),
            timestamp: OffsetDateTime::now_utc(),
            fcm_token: "".to_owned(),
        }
    }
}

pub fn all(conn: &mut PgConnection) -> QueryResult<Vec<User>> {
    users::dsl::users.load(conn)
}
pub fn by_id(conn: &mut PgConnection, id: String) -> QueryResult<Option<User>> {
    let x = users::table
        .filter(users::pubkey.eq(id))
        .first(conn)
        .optional()?;

    Ok(x)
}

pub fn insert(conn: &mut PgConnection, user: User) -> QueryResult<User> {
    let user: User = diesel::insert_into(users::table)
        .values(&user)
        .get_result(conn)?;
    Ok(user)
}

pub fn update_fcm_token(conn: &mut PgConnection, params: TokenUpdateParams) -> Result<()> {
    let fcm_token = &params.fcm_token;
    let pubkey = params.pubkey;
    tracing::debug!(%pubkey, "Updating token for client.");

    let affected_rows = diesel::update(users::table)
        .filter(users::pubkey.eq(&pubkey))
        .set(users::fcm_token.eq(fcm_token))
        .execute(conn)?;

    if affected_rows == 0 {
        bail!("Could not update FCM token for node ID {pubkey}.");
    } else {
        tracing::debug!(%pubkey, %affected_rows, "Updated FCM token in DB.");
    }
    Ok(())
}
