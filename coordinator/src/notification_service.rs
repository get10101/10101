use anyhow::Context;
use anyhow::Result;
use std::fmt::Display;
use tokio::sync::mpsc;

/// Types of notification that can be sent to 10101 app users

#[derive(Debug, Clone)]
pub enum NotificationKind {
    /// Coordinator would like to settle the channel
    ChannelClose,
    PositionSoonToExpire,
    PositionExpired,
}

impl Display for NotificationKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NotificationKind::ChannelClose => write!(f, "ChannelClose"),
            NotificationKind::PositionSoonToExpire => write!(f, "PositionSoonToExpire"),
            NotificationKind::PositionExpired => write!(f, "PositionExpired"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Notification {
    pub user_fcm_token: String,
    pub notification_kind: NotificationKind,
}

impl Notification {
    pub fn new(user_fcm_token: String, notification_kind: NotificationKind) -> Self {
        Self {
            notification_kind,
            user_fcm_token,
        }
    }
}

/// Actor managing the notifications
pub struct NotificationService {
    notification_sender: mpsc::Sender<Notification>,
}

impl NotificationService {
    /// Start the notification service
    ///
    /// If an empty string is passed in the constructor, the service will not send any notification.
    /// It will only log the notification that it would have sent.
    pub fn new(fcm_api_key: String) -> Self {
        if fcm_api_key.is_empty() {
            // Log it as error, as in production it should always be set
            tracing::error!("FCM API key is empty. No notifications will not be sent.");
        }

        let (notification_sender, mut notification_receiver) = mpsc::channel(100);

        // TODO: use RAII here
        tokio::spawn({
            let fcm_api_key = fcm_api_key;
            let client = fcm::Client::new();
            async move {
                while let Some(Notification {
                    user_fcm_token,
                    notification_kind,
                }) = notification_receiver.recv().await
                {
                    tracing::info!(%notification_kind, %user_fcm_token, "Sending notification");

                    if !fcm_api_key.is_empty() {
                        let notification = build_notification(notification_kind);
                        if let Err(e) =
                            send_notification(&client, &fcm_api_key, &user_fcm_token, notification)
                                .await
                        {
                            tracing::error!("Could not send notification to FCM: {:?}", e);
                        }
                    }
                }
            }
        });

        Self {
            notification_sender,
        }
    }

    /// Constructs a new sender. Use a sender to send notification from any part of the system.
    pub fn get_sender(&self) -> mpsc::Sender<Notification> {
        self.notification_sender.clone()
    }
}

/// Prepares the notification text
fn build_notification<'a>(kind: NotificationKind) -> fcm::Notification<'a> {
    let mut notification_builder = fcm::NotificationBuilder::new();
    match kind {
        NotificationKind::ChannelClose => {
            notification_builder.body("Close channel request.");
            notification_builder.title("Someone wants to close a position with you! 🌻");
        }
        NotificationKind::PositionSoonToExpire => {
            notification_builder.title("Your Position is about to expire");
            notification_builder.body("Open the app to react.");
        }
        NotificationKind::PositionExpired => {
            notification_builder.title("Your position has expired");
            notification_builder.body("Open the app to react.");
        }
    }
    notification_builder.finalize()
}

async fn send_notification<'a>(
    client: &fcm::Client,
    api_key: &str,
    fcm_token: &str,
    notification: fcm::Notification<'a>,
) -> Result<()> {
    anyhow::ensure!(!api_key.is_empty(), "FCM API key is empty");

    let mut message_builder = fcm::MessageBuilder::new(api_key, fcm_token);
    message_builder.notification(notification);
    let message = message_builder.finalize();
    let response = client
        .send(message)
        .await
        .context("could not send FCM notification")?;
    tracing::debug!("Sent: {:?}", response);
    Ok(())
}
