use crate::db::user;
use crate::notifications::FcmToken;
use crate::notifications::Notification;
use crate::notifications::NotificationKind;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::PgConnection;
use futures::future::RemoteHandle;
use futures::FutureExt;
use orderbook_commons::Message;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::mpsc;

/// This value is arbitrarily set to 100 and defines the message accepted in the message
/// channel buffer.
const NOTIFICATION_BUFFER_SIZE: usize = 100;

/// Message sent to users via the websocket.
pub enum OrderbookMessage {
    TraderMessage {
        trader_id: PublicKey,
        message: Message,
        notification: Option<NotificationKind>,
    },
}

#[derive(Clone)]
pub struct NewUserMessage {
    pub new_user: PublicKey,
    pub sender: mpsc::Sender<Message>,
}

pub fn spawn_delivering_messages_to_authenticated_users(
    pool: Pool<ConnectionManager<PgConnection>>,
    notification_sender: mpsc::Sender<Notification>,
    tx_user_feed: broadcast::Sender<NewUserMessage>,
) -> (RemoteHandle<Result<()>>, mpsc::Sender<OrderbookMessage>) {
    let (sender, mut receiver) = mpsc::channel::<OrderbookMessage>(NOTIFICATION_BUFFER_SIZE);

    let authenticated_users = Arc::new(RwLock::new(HashMap::new()));

    tokio::task::spawn({
        let traders = authenticated_users.clone();
        async move {
            let mut user_feed = tx_user_feed.subscribe();
            while let Ok(new_user_msg) = user_feed.recv().await {
                traders
                    .write()
                    .insert(new_user_msg.new_user, new_user_msg.sender);
            }
        }
    });

    let (fut, remote_handle) = {
        async move {
            while let Some(notification) = receiver.recv().await {
                let mut conn = pool.get()?;
                match notification {
                    OrderbookMessage::TraderMessage { trader_id, message , notification} => {
                        tracing::info!(%trader_id, "Sending trader message: {message:?}");

                        let trader = authenticated_users.read().get(&trader_id).cloned();

                        match trader {
                            Some(sender) => {
                                if let Err(e) = sender.send(message).await {
                                    tracing::warn!(%trader_id, "Connection lost to trader {e:#}");
                                } else {
                                    tracing::trace!(%trader_id, "Skipping optional push notifications as the user was successfully notified via the websocket.");
                                    continue;
                                }
                            }
                            None => tracing::warn!(%trader_id, "Trader is not connected."),
                        };

                        if let (Some(notification_kind),Some(user)) = (notification, user::by_id(&mut conn, trader_id.to_string())?) {
                            tracing::debug!(%trader_id, "Sending push notification to user");

                            match FcmToken::new(user.fcm_token) {
                                Ok(fcm_token) => {
                                    if let Err(e) = notification_sender
                                        .send(Notification {
                                            user_fcm_token: fcm_token,
                                            notification_kind,
                                        })
                                        .await {
                                        tracing::error!(%trader_id, "Failed to send push notification. Error: {e:#}");
                                    }
                                }
                                Err(error) => {
                                    tracing::error!(%trader_id, "Could not send notification to user. Error: {error:#}");
                                }
                            }
                        }
                    }
                }
            }

            Ok(())
        }
        .remote_handle()
    };

    tokio::spawn(fut);

    (remote_handle, sender)
}
