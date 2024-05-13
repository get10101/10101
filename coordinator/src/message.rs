use crate::notifications::Notification;
use crate::notifications::NotificationKind;
use anyhow::Context;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use futures::future::RemoteHandle;
use futures::FutureExt;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;
use xxi_node::commons::Message;

/// This value is arbitrarily set to 100 and defines the message accepted in the message
/// channel buffer.
const NOTIFICATION_BUFFER_SIZE: usize = 100;

/// Message sent to users via the websocket.
#[derive(Debug)]
pub struct TraderMessage {
    pub trader_id: PublicKey,
    pub message: Message,
    pub notification: Option<NotificationKind>,
}

#[derive(Clone)]
pub struct NewUserMessage {
    pub new_user: PublicKey,
    pub sender: Sender<Message>,
}

pub fn spawn_delivering_messages_to_authenticated_users(
    notification_sender: Sender<Notification>,
    tx_user_feed: broadcast::Sender<NewUserMessage>,
) -> (RemoteHandle<()>, Sender<TraderMessage>) {
    let (sender, mut receiver) = mpsc::channel::<TraderMessage>(NOTIFICATION_BUFFER_SIZE);

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
            while let Some(trader_message) = receiver.recv().await {
                if let Err(e) = process_trader_message(
                    &authenticated_users,
                    &notification_sender,
                    trader_message,
                )
                .await
                {
                    tracing::error!("Failed to process trader message: {e:#}");
                }
            }

            tracing::error!("Channel closed");
        }
        .remote_handle()
    };

    tokio::spawn(fut);

    (remote_handle, sender)
}

async fn process_trader_message(
    authenticated_users: &RwLock<HashMap<PublicKey, Sender<Message>>>,
    notification_sender: &Sender<Notification>,
    trader_message: TraderMessage,
) -> Result<()> {
    let trader_id = trader_message.trader_id;
    let message = trader_message.message;
    let notification = trader_message.notification;
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

    if let Some(notification_kind) = notification {
        tracing::debug!(%trader_id, "Sending push notification to user");

        notification_sender
            .send(Notification::new(trader_id, notification_kind))
            .await
            .with_context(|| format!("Failed to send push notification to trader {trader_id}"))?;
    }

    Ok(())
}
