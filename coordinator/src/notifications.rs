use anyhow::Context;
use anyhow::Result;
use std::fmt::Display;
use tokio::sync::mpsc;

/// Types of notification that can be sent to 10101 app users

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotificationKind {
    RolloverWindowOpen,
    PositionSoonToExpire,
    PositionExpired,
}

impl Display for NotificationKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NotificationKind::PositionSoonToExpire => write!(f, "PositionSoonToExpire"),
            NotificationKind::PositionExpired => write!(f, "PositionExpired"),
            NotificationKind::RolloverWindowOpen => write!(f, "RolloverWindowOpen"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Notification {
    pub user_fcm_token: FcmToken,
    pub notification_kind: NotificationKind,
}

impl Notification {
    pub fn new(user_fcm_token: FcmToken, notification_kind: NotificationKind) -> Self {
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
        NotificationKind::PositionSoonToExpire => {
            notification_builder.title("Your position is about to expire");
            notification_builder.body("Rollover your position for the next cycle.");
        }
        NotificationKind::PositionExpired => {
            notification_builder.title("Your position has expired");
            notification_builder.body("Close your position.");
        }
        NotificationKind::RolloverWindowOpen => {
            notification_builder.title("Rollover window is open");
            notification_builder.body("Rollover your position for the next cycle.");
        }
    }
    notification_builder.finalize()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FcmToken(String);

impl FcmToken {
    pub fn new(token: String) -> Result<Self> {
        anyhow::ensure!(!token.is_empty(), "FCM token cannot be empty");
        Ok(Self(token))
    }

    pub fn get(&self) -> &str {
        &self.0
    }
}

impl Display for FcmToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", &self.0)
    }
}

async fn send_notification<'a>(
    client: &fcm::Client,
    api_key: &str,
    fcm_token: &FcmToken,
    notification: fcm::Notification<'a>,
) -> Result<()> {
    anyhow::ensure!(!api_key.is_empty(), "FCM API key is empty");

    let mut message_builder = fcm::MessageBuilder::new(api_key, fcm_token.get());
    message_builder.notification(notification);
    let message = message_builder.finalize();
    let response = client
        .send(message)
        .await
        .context("could not send FCM notification")?;
    tracing::debug!("Sent notification. Response: {:?}", response);
    Ok(())
}
