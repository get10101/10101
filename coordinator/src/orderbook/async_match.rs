use crate::message::NewUserMessage;
use crate::message::OrderbookMessage;
use crate::orderbook::db::matches;
use crate::orderbook::db::orders;
use anyhow::ensure;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use bitcoin::Network;
use bitcoin::XOnlyPublicKey;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::PgConnection;
use futures::future::RemoteHandle;
use futures::FutureExt;
use orderbook_commons::FilledWith;
use orderbook_commons::Match;
use orderbook_commons::Matches;
use orderbook_commons::Message;
use orderbook_commons::OrderReason;
use orderbook_commons::OrderState;
use time::OffsetDateTime;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc;

pub fn monitor(
    pool: Pool<ConnectionManager<PgConnection>>,
    tx_user_feed: broadcast::Sender<NewUserMessage>,
    notifier: mpsc::Sender<OrderbookMessage>,
    network: Network,
    oracle_pk: XOnlyPublicKey,
) -> RemoteHandle<Result<()>> {
    let mut user_feed = tx_user_feed.subscribe();
    let (fut, remote_handle) = async move {
        loop {
            match user_feed.recv().await {
                Ok(new_user_msg) => {
                    tokio::spawn({
                        let mut conn = pool.get()?;
                        let notifier = notifier.clone();
                        async move {
                            tracing::debug!(trader_id=%new_user_msg.new_user, "Checking if the user needs to be notified about pending matches");
                            if let Err(e) = process_pending_match(&mut conn, notifier, new_user_msg.new_user, network, oracle_pk).await {
                                tracing::error!("Failed to process pending match. Error: {e:#}");
                            }
                        }
                    });
                }
                Err(RecvError::Closed) => {
                    tracing::error!("New user message sender died! Channel closed.");
                    break;
                }
                Err(RecvError::Lagged(skip)) => tracing::warn!(%skip,
                        "Lagging behind on new user message."
                    ),
            }
        }
        Ok(())
    }.remote_handle();

    tokio::spawn(fut);

    remote_handle
}

/// Checks if there are any pending matches
async fn process_pending_match(
    conn: &mut PgConnection,
    notifier: mpsc::Sender<OrderbookMessage>,
    trader_id: PublicKey,
    network: Network,
    oracle_pk: XOnlyPublicKey,
) -> Result<()> {
    if let Some(order) = orders::get_by_trader_id_and_state(conn, trader_id, OrderState::Matched)? {
        tracing::debug!(%trader_id, order_id=%order.id, "Notifying trader about pending match");

        let matches = matches::get_matches_by_order_id(conn, order.id)?;
        let filled_with = get_filled_with_from_matches(matches, network, oracle_pk)?;

        let message = match order.order_reason {
            OrderReason::Manual => Message::Match(filled_with),
            OrderReason::Expired => Message::AsyncMatch { order, filled_with },
        };

        // Sending no optional push notification as this is only executed if the user just
        // registered on the websocket. So we can assume that the user is still online.
        let notification = None;
        let msg = OrderbookMessage::TraderMessage {
            trader_id,
            message,
            notification,
        };
        if let Err(e) = notifier.send(msg).await {
            tracing::error!("Failed to send notification. Error: {e:#}");
        }
    }

    Ok(())
}

fn get_filled_with_from_matches(
    matches: Vec<Matches>,
    network: Network,
    oracle_pk: XOnlyPublicKey,
) -> Result<FilledWith> {
    ensure!(
        !matches.is_empty(),
        "Need at least one matches record to construct a FilledWith"
    );

    let order_id = matches
        .first()
        .expect("to have at least one match")
        .order_id;

    let expiry_timestamp =
        coordinator_commons::calculate_next_expiry(OffsetDateTime::now_utc(), network);

    Ok(FilledWith {
        order_id,
        expiry_timestamp,
        oracle_pk,
        matches: matches
            .iter()
            .map(|m| Match {
                id: m.id,
                order_id: m.order_id,
                quantity: m.quantity,
                pubkey: m.match_trader_id,
                execution_price: m.execution_price,
            })
            .collect(),
    })
}
