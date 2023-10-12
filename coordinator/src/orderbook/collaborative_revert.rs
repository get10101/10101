use crate::db::channels;
use crate::db::collaborative_reverts;
use crate::message::NewUserMessage;
use crate::message::OrderbookMessage;
use crate::position::models::parse_channel_id;
use anyhow::bail;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::PgConnection;
use futures::future::RemoteHandle;
use futures::FutureExt;
use orderbook_commons::Message;
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
    if let Some(channel) = channels::get_by_trader_pubkey(trader_id, conn)? {
        if channel.channel_id.is_none() {
            bail!("Can't revert a channel which does not have a channel_id");
        }
        let channel_id = channel.channel_id.expect("To exist");
        tracing::debug!(%trader_id, channel_id, "Notifying trader about pending collaborative revert");

        match collaborative_reverts::get(channel_id.as_str(), conn)? {
            None => {
                tracing::warn!("No pending collaborative revert for user {trader_id}");
                return Ok(());
            }
            Some(collaborative_reverts) => {
                // Sending no optional push notification as this is only executed if the user just
                // registered on the websocket. So we can assume that the user is still online.
                let msg = OrderbookMessage::CollaborativeRevert {
                    trader_id,
                    message: Message::CollaborativeRevert {
                        channel_id: parse_channel_id(channel_id.as_str())?,
                        coordinator_address: collaborative_reverts.coordinator_address,
                        coordinator_amount: collaborative_reverts.coordinator_amount_sats,
                        trader_amount: collaborative_reverts.trader_amount_sats,
                    },
                };
                if let Err(e) = notifier.send(msg).await {
                    bail!("Failed to send notification. Error: {e:#}");
                }
            }
        }
    }

    Ok(())
}
