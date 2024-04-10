use crate::db::user;
use crate::notifications::FcmToken;
use crate::orderbook;
use commons::OrderReason;
use diesel::Connection;
use diesel::PgConnection;
use diesel::QueryResult;

pub fn get_all_matched_market_orders_by_order_reason(
    conn: &mut PgConnection,
    order_reasons: Vec<OrderReason>,
) -> QueryResult<Vec<(commons::Order, FcmToken)>> {
    let result = conn.transaction(|conn| {
        let orders = orderbook::db::orders::get_all_matched_market_orders_by_order_reason(
            conn,
            order_reasons,
        )?;
        join_with_fcm_token(conn, orders)
    })?;

    Ok(result)
}

pub fn join_with_fcm_token(
    conn: &mut PgConnection,
    orders: Vec<commons::Order>,
) -> QueryResult<Vec<(commons::Order, FcmToken)>> {
    let users = user::all(conn)?;
    let result = orders
        .into_iter()
        // Join orders with users to add the FCM tokens.
        // Filter out orders that don't have a FCM token stored in the users
        // table which is with them.
        // This can be done at the DB level if it ever becomes a performance issue.
        .filter_map(|o| {
            let maybe_fcm_token = users
                .iter()
                .find(|u| u.pubkey == o.trader_id.to_string() && !u.fcm_token.is_empty())
                .map(|u| FcmToken::new(u.fcm_token.clone()).expect("To have a non-empty token."));

            if let Some(fcm_token) = maybe_fcm_token {
                Some((o, fcm_token))
            } else {
                tracing::warn!(?o, "No FCM token for order");
                None
            }
        })
        .collect::<Vec<_>>();
    diesel::result::QueryResult::Ok(result)
}
