use crate::db::collaborative_reverts;
use crate::message::NewUserMessage;
use crate::message::OrderbookMessage;
use anyhow::bail;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use bitcoin::OutPoint;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::PgConnection;
use futures::future::RemoteHandle;
use futures::FutureExt;
use orderbook_commons::Message;
use rust_decimal::Decimal;
use tokio::sync::broadcast;
use tokio::sync::mpsc;

pub fn monitor(
    pool: Pool<ConnectionManager<PgConnection>>,
    tx_user_feed: broadcast::Sender<NewUserMessage>,
    notifier: mpsc::Sender<OrderbookMessage>,
) -> RemoteHandle<Result<()>> {
    let mut user_feed = tx_user_feed.subscribe();
    let (fut, remote_handle) = async move {
        while let Ok(new_user_msg) = user_feed.recv().await {
            tokio::spawn({
                let mut conn = pool.get()?;
                let notifier = notifier.clone();
                async move {
                    tracing::debug!(trader_id=%new_user_msg.new_user, "Checking if the user needs to be notified about collaboratively reverting a channel");
                    if let Err(e) = process_pending_collaborative_revert(&mut conn, notifier, new_user_msg.new_user).await {
                        tracing::error!("Failed to process pending collaborative revert. Error: {e:#}");
                    }
                }
            });
        }
        Ok(())
    }.remote_handle();

    tokio::spawn(fut);

    remote_handle
}

/// Checks if there are any pending collaborative reverts
async fn process_pending_collaborative_revert(
    conn: &mut PgConnection,
    notifier: mpsc::Sender<OrderbookMessage>,
    trader_id: PublicKey,
) -> Result<()> {
    match collaborative_reverts::by_trader_pubkey(trader_id.to_string().as_str(), conn)? {
        None => {
            // nothing to revert
        }
        Some(revert) => {
            tracing::debug!(%trader_id, channel_id = hex::encode(revert.channel_id), "Notifying trader about pending collaborative revert");

            // Sending no optional push notification as this is only executed if the user just
            // registered on the websocket. So we can assume that the user is still online.
            let msg = OrderbookMessage::TraderMessage {
                trader_id,
                message: Message::CollaborativeRevert {
                    channel_id: revert.channel_id,
                    coordinator_address: revert.coordinator_address,
                    coordinator_amount: revert.coordinator_amount_sats,
                    trader_amount: revert.trader_amount_sats,
                    execution_price: Decimal::try_from(revert.price).expect("to fit into decimal"),
                    outpoint: OutPoint {
                        txid: revert.txid,
                        vout: revert.vout,
                    },
                },
                notification: None,
            };
            if let Err(e) = notifier.send(msg).await {
                bail!("Failed to send notification. Error: {e:#}");
            }
        }
    }

    Ok(())
}
