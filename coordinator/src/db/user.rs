use crate::db::bonus_status;
use crate::db::bonus_status::BonusType;
use crate::db::bonus_tiers;
use crate::schema;
use crate::schema::users;
use anyhow::bail;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use commons::referral_from_pubkey;
use commons::RegisterParams;
use diesel::prelude::*;
use serde::Deserialize;
use serde::Serialize;
use time::OffsetDateTime;

#[derive(Queryable, Identifiable, Debug, Clone, Serialize, Deserialize)]
#[diesel(primary_key(id))]
pub struct User {
    #[diesel(deserialize_as = i32)]
    pub id: Option<i32>,
    pub pubkey: String,
    pub contact: String,
    pub timestamp: OffsetDateTime,
    pub fcm_token: String,
    pub last_login: OffsetDateTime,
    pub nickname: Option<String>,
    // TODO(holzeis): Version is only optional for the first upgrade. Afterwards we should make it
    // mandatory.
    pub version: Option<String>,
    /// personal referral code
    pub referral_code: String,
    /// The referral code referred by
    pub used_referral_code: Option<String>,
}

#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(primary_key(id), table_name = users)]
pub struct NewUser {
    #[diesel(deserialize_as = i32)]
    pub id: Option<i32>,
    pub pubkey: String,
    pub contact: String,
    pub timestamp: OffsetDateTime,
    pub fcm_token: String,
    pub last_login: OffsetDateTime,
    pub nickname: Option<String>,
    // TODO(holzeis): Version is only optional for the first upgrade. Afterwards we should make it
    // mandatory.
    pub version: Option<String>,
    /// This user was referred by this code
    pub used_referral_code: Option<String>,
}

impl From<RegisterParams> for User {
    fn from(value: RegisterParams) -> Self {
        let referral_code = referral_from_pubkey(value.pubkey);

        User {
            id: None,
            pubkey: value.pubkey.to_string(),
            contact: value.contact.unwrap_or("".to_owned()),
            nickname: value.nickname,
            timestamp: OffsetDateTime::now_utc(),
            fcm_token: "".to_owned(),
            last_login: OffsetDateTime::now_utc(),
            version: value.version,
            // TODO: this is not ideal, we shouldn't need to do this as it's autogenerated in the db
            // However, this is needed here because we convert from `RegisteredUser` to `User`. We
            // should not do this anymore.
            referral_code,
            used_referral_code: value.referral_code,
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

/// Returns all users which have logged in since the `cut_off`
pub fn all_with_login_date(
    conn: &mut PgConnection,
    cut_off: OffsetDateTime,
) -> QueryResult<Vec<User>> {
    users::dsl::users
        .filter(users::last_login.ge(cut_off))
        .load(conn)
}

pub fn upsert_user(
    conn: &mut PgConnection,
    trader_id: PublicKey,
    contact: Option<String>,
    nickname: Option<String>,
    version: Option<String>,
    used_referral_code: Option<String>,
) -> QueryResult<User> {
    // If no name or contact has been provided we default to empty string
    let contact = contact.unwrap_or_default();

    let timestamp = OffsetDateTime::now_utc();

    let user: User = diesel::insert_into(users::table)
        .values(NewUser {
            id: None,
            pubkey: trader_id.to_string(),
            contact: contact.clone(),
            nickname: nickname.clone(),
            timestamp,
            fcm_token: "".to_owned(),
            last_login: timestamp,
            version: version.clone(),
            used_referral_code: used_referral_code.clone(),
        })
        .on_conflict(schema::users::pubkey)
        .do_update()
        .set((
            users::contact.eq(&contact),
            users::nickname.eq(&nickname),
            users::last_login.eq(timestamp),
            users::version.eq(version),
        ))
        .get_result(conn)?;

    if let Some(referral_code) = used_referral_code {
        // we need to check if this referral code is sane
        if let Ok(Some(_)) = get_user_for_referral(conn, referral_code.as_str()) {
            let bonus_tier = bonus_tiers::all_active_by_type(conn, vec![BonusType::Referent])?;
            let bonus_tier = bonus_tier.first().expect("to have at least one tier");
            bonus_status::insert(conn, &trader_id, bonus_tier.tier_level, BonusType::Referent)?;
        } else {
            tracing::warn!(
                referral_code,
                trader_pubkey = trader_id.to_string(),
                "User tried to register with invalid referral code"
            )
        }
    }

    Ok(user)
}

pub fn update_nickname(
    conn: &mut PgConnection,
    trader_id: PublicKey,
    nickname: Option<String>,
) -> QueryResult<()> {
    let nickname = nickname.unwrap_or_default();

    let updated_rows = diesel::update(users::table)
        .filter(users::pubkey.eq(trader_id.to_string()))
        .set(users::nickname.eq(nickname.clone()))
        .execute(conn)?;

    if updated_rows == 0 {
        tracing::warn!(
            trader_id = trader_id.to_string(),
            nickname,
            "No username updated"
        )
    }

    Ok(())
}

pub fn login_user(
    conn: &mut PgConnection,
    trader_id: PublicKey,
    token: String,
    version: Option<String>,
) -> Result<()> {
    tracing::debug!(%trader_id, token, "Updating token for client.");
    let last_login = OffsetDateTime::now_utc();
    let affected_rows = diesel::insert_into(users::table)
        .values(NewUser {
            id: None,
            pubkey: trader_id.to_string(),
            contact: "".to_owned(),
            nickname: None,
            timestamp: OffsetDateTime::now_utc(),
            fcm_token: token.clone(),
            version: version.clone(),
            last_login,
            // TODO: this breaks the used referral code
            used_referral_code: None,
        })
        .on_conflict(schema::users::pubkey)
        .do_update()
        .set((
            users::fcm_token.eq(&token),
            users::last_login.eq(last_login),
            users::version.eq(version),
        ))
        .execute(conn)?;

    if affected_rows == 0 {
        bail!("Could not update FCM token for node ID {trader_id}.");
    } else {
        tracing::debug!(%trader_id, %affected_rows, "Updated FCM token in DB.");
    }
    Ok(())
}

pub fn get_user(conn: &mut PgConnection, trader_id: &PublicKey) -> Result<Option<User>> {
    let maybe_user = users::table
        .filter(users::pubkey.eq(trader_id.to_string()))
        .first(conn)
        .optional()?;

    Ok(maybe_user)
}

pub fn get_users(conn: &mut PgConnection, trader_ids: Vec<PublicKey>) -> Result<Vec<User>> {
    let users = users::table
        .filter(users::pubkey.eq_any(trader_ids.iter().map(|id| id.to_string())))
        .load(conn)?;

    Ok(users)
}

pub fn get_referred_users(conn: &mut PgConnection, referral_code: String) -> Result<Vec<User>> {
    let users = users::table
        .filter(users::used_referral_code.eq(referral_code))
        .load(conn)?;

    Ok(users)
}
pub fn get_user_for_referral(conn: &mut PgConnection, referral_code: &str) -> Result<Option<User>> {
    let user = users::table
        .filter(users::referral_code.eq(referral_code))
        .first(conn)
        .optional()?;

    Ok(user)
}
