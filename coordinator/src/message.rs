use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
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
    },
}

#[derive(Clone)]
pub struct NewUserMessage {
    pub new_user: PublicKey,
    pub sender: mpsc::Sender<Message>,
}

pub fn spawn_delivering_messages_to_authenticated_users(
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
                match notification {
                    OrderbookMessage::TraderMessage { trader_id, message } => {
                        tracing::info!(%trader_id, "Sending message: {message:?}");

                        let trader = authenticated_users.read().get(&trader_id).cloned();

                        match trader {
                            Some(sender) => {
                                if let Err(e) = sender.send(message).await {
                                    tracing::warn!("Connection lost to trader {e:#}");
                                }
                            }
                            None => tracing::warn!(%trader_id, "Trader is not connected"),
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
