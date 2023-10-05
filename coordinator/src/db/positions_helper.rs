use crate::db::positions::Position;
use crate::db::user;
use crate::notifications::FcmToken;
use diesel::Connection;
use diesel::PgConnection;
use diesel::QueryResult;
use time::OffsetDateTime;

pub fn get_all_open_positions_with_expiry_before(
    conn: &mut PgConnection,
    expiry: OffsetDateTime,
) -> QueryResult<Vec<(crate::position::models::Position, FcmToken)>> {
    let result = conn.transaction(|conn| {
        let positions = Position::get_all_open_positions_with_expiry_before(conn, expiry)?;
        join_with_fcm_token(conn, positions)
    })?;
    Ok(result)
}

pub fn get_positions_joined_with_fcm_token_with_expiry_within(
    conn: &mut PgConnection,
    start: OffsetDateTime,
    end: OffsetDateTime,
) -> QueryResult<Vec<(crate::position::models::Position, FcmToken)>> {
    let result = conn.transaction(|conn| {
        let positions = Position::get_all_positions_with_expiry_within(conn, start, end)?;
        join_with_fcm_token(conn, positions)
    })?;
    Ok(result)
}

pub fn join_with_fcm_token(
    conn: &mut PgConnection,
    positions: Vec<crate::position::models::Position>,
) -> QueryResult<Vec<(crate::position::models::Position, FcmToken)>> {
    let users = user::all(conn)?;
    let result = positions
        .into_iter()
        // Join positions with users to add the FCM tokens.
        // Filter out positions that don't have a FCM token stored in the users
        // table which is with them.
        // This can be done at the DB level if it ever becomes a performance issue.
        .filter_map(|p| {
            let maybe_fcm_token = users
                .iter()
                .find(|u| u.pubkey == p.trader.to_string() && !u.fcm_token.is_empty())
                .map(|u| FcmToken::new(u.fcm_token.clone()).expect("To have a non-empty token."));

            if let Some(fcm_token) = maybe_fcm_token {
                Some((p, fcm_token))
            } else {
                tracing::warn!(?p, "No FCM token for position");
                None
            }
        })
        .collect::<Vec<_>>();
    diesel::result::QueryResult::Ok(result)
}
