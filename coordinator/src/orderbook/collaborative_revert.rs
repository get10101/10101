use crate::db::collaborative_reverts;
use crate::message::NewUserMessage;
use crate::message::OrderbookMessage;
use anyhow::bail;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use bitcoin::Address;
use bitcoin::Network;
use commons::Message;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::PgConnection;
use futures::future::RemoteHandle;
use futures::FutureExt;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc;
use tokio::task::spawn_blocking;

pub fn monitor(
    pool: Pool<ConnectionManager<PgConnection>>,
    tx_user_feed: broadcast::Sender<NewUserMessage>,
    notifier: mpsc::Sender<OrderbookMessage>,
    network: Network,
) -> RemoteHandle<()> {
    let mut user_feed = tx_user_feed.subscribe();
    let (fut, remote_handle) = async move {
        loop {
            match user_feed.recv().await {
                Ok(new_user_msg) => {
                    tokio::spawn({
                        let notifier = notifier.clone();
                        let pool = pool.clone();
                        async move {
                            tracing::debug!(
                                trader_id=%new_user_msg.new_user,
                                "Checking if the user needs to be notified about \
                                 collaboratively reverting a channel"
                            );

                            if let Err(e) = process_pending_collaborative_revert(
                                pool,
                                notifier,
                                new_user_msg.new_user,
                                network,
                            )
                            .await
                            {
                                tracing::error!(
                                    "Failed to process pending collaborative revert. Error: {e:#}"
                                );
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
    }
    .remote_handle();

    tokio::spawn(fut);

    remote_handle
}

/// Checks if there are any pending collaborative reverts
async fn process_pending_collaborative_revert(
    pool: Pool<ConnectionManager<PgConnection>>,
    notifier: mpsc::Sender<OrderbookMessage>,
    trader_id: PublicKey,
    network: Network,
) -> Result<()> {
    let mut conn = spawn_blocking(move || pool.get())
        .await
        .expect("task to complete")?;

    match collaborative_reverts::by_trader_pubkey(
        trader_id.to_string().as_str(),
        network,
        &mut conn,
    )? {
        None => {
            // nothing to revert
        }
        Some(revert) => {
            tracing::debug!(
                %trader_id,
                channel_id = hex::encode(revert.channel_id),
                "Notifying trader about pending collaborative revert"
            );

            // Sending no optional push notification as this is only executed if the user just
            // registered on the websocket. So we can assume that the user is still online.
            let msg = OrderbookMessage::TraderMessage {
                trader_id,
                message: Message::DlcChannelCollaborativeRevert {
                    channel_id: revert.channel_id,
                    coordinator_address: Address::new(
                        revert.coordinator_address.network,
                        revert.coordinator_address.payload,
                    ),
                    coordinator_amount: revert.coordinator_amount_sats,
                    trader_amount: revert.trader_amount_sats,
                    execution_price: revert.price,
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
