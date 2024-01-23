use crate::db::user;
use crate::notifications::FcmToken;
use crate::notifications::Notification;
use crate::notifications::NotificationKind;
use anyhow::Context;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use commons::Message;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::PgConnection;
use futures::future::RemoteHandle;
use futures::FutureExt;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;
use tokio::task::spawn_blocking;

/// This value is arbitrarily set to 100 and defines theff message accepted in the message
/// channel buffer.
const NOTIFICATION_BUFFER_SIZE: usize = 100;

/// Message sent to users via the websocket.
#[derive(Debug)]
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
    pub sender: Sender<Message>,
}

pub fn spawn_delivering_messages_to_authenticated_users(
    pool: Pool<ConnectionManager<PgConnection>>,
    notification_sender: Sender<Notification>,
    tx_user_feed: broadcast::Sender<NewUserMessage>,
) -> (RemoteHandle<()>, Sender<OrderbookMessage>) {
    let (sender, mut receiver) = mpsc::channel::<OrderbookMessage>(NOTIFICATION_BUFFER_SIZE);

    let authenticated_users = Arc::new(RwLock::new(HashMap::new()));

    tokio::task::spawn({
        let traders = authenticated_users.clone();
        async move {
            let mut user_feed = tx_user_feed.subscribe();
            loop {
                match user_feed.recv().await {
                    Ok(new_user_msg) => {
                        traders
                            .write()
                            .insert(new_user_msg.new_user, new_user_msg.sender);
                    }
                    Err(RecvError::Closed) => {
                        tracing::error!("New user message sender died! Channel closed");
                        break;
                    }
                    Err(RecvError::Lagged(skip)) => {
                        tracing::warn!(%skip, "Lagging behind on new user message")
                    }
                }
            }
        }
    });

    let (fut, remote_handle) = {
        async move {
            while let Some(notification) = receiver.recv().await {
                if let Err(e) = process_orderbook_message(
                    pool.clone(),
                    &authenticated_users,
                    &notification_sender,
                    notification,
                )
                .await
                {
                    tracing::error!("Failed to process orderbook message: {e:#}");
                }
            }

            tracing::error!("Channel closed");
        }
        .remote_handle()
    };

    tokio::spawn(fut);

    (remote_handle, sender)
}

async fn process_orderbook_message(
    pool: Pool<ConnectionManager<PgConnection>>,
    authenticated_users: &RwLock<HashMap<PublicKey, Sender<Message>>>,
    notification_sender: &Sender<Notification>,
    notification: OrderbookMessage,
) -> Result<()> {
    let mut conn = spawn_blocking(move || pool.get())
        .await
        .expect("task to complete")?;

    match notification {
        OrderbookMessage::TraderMessage {
            trader_id,
            message,
            notification,
        } => {
            tracing::info!(%trader_id, ?message, "Sending trader message");

            let trader = authenticated_users.read().get(&trader_id).cloned();

            match trader {
                Some(sender) => {
                    if let Err(e) = sender.send(message).await {
                        tracing::warn!(%trader_id, "Connection lost to trader: {e:#}");
                    } else {
                        tracing::trace!(
                            %trader_id,
                            "Skipping optional push notifications as the user was successfully \
                             notified via the websocket"
                        );
                        return Ok(());
                    }
                }
                None => tracing::warn!(%trader_id, "Trader is not connected"),
            };

            let user = user::by_id(&mut conn, trader_id.to_string())
                .context("Failed to get user by ID")?;

            if let (Some(notification_kind), Some(user)) = (notification, user) {
                tracing::debug!(%trader_id, "Sending push notification to user");

                let fcm_token = FcmToken::new(user.fcm_token)?;

                notification_sender
                    .send(Notification {
                        user_fcm_token: fcm_token,
                        notification_kind,
                    })
                    .await
                    .with_context(|| {
                        format!("Failed to send push notification to trader {trader_id}")
                    })?;
            }
        }
    }

    Ok(())
}
