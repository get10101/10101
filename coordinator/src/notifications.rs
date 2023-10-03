use crate::db::positions_helper::get_positions_joined_with_fcm_token_with_expiry_within;
use crate::position::models::Position;
use anyhow::Context;
use anyhow::Result;
use diesel::PgConnection;
use std::fmt::Display;
use time::OffsetDateTime;
use tokio::sync::mpsc;

/// A position expiring soon if it expires in less than this time
const START_OF_EXPIRING_POSITION: time::Duration = time::Duration::hours(13);

/// A position 'expiring soon' if it expires in less than this time
const END_OF_EXPIRING_POSITION: time::Duration = time::Duration::hours(12);

/// A position expired if it expired more than this time ago
const END_OF_EXPIRED_POSITION: time::Duration = time::Duration::hours(1);

/// Types of notification that can be sent to 10101 app users

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotificationKind {
    PositionSoonToExpire,
    PositionExpired,
}

impl Display for NotificationKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NotificationKind::PositionSoonToExpire => write!(f, "PositionSoonToExpire"),
            NotificationKind::PositionExpired => write!(f, "PositionExpired"),
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
            notification_builder.body("Open the app to react.");
        }
        NotificationKind::PositionExpired => {
            notification_builder.title("Your position has expired");
            notification_builder.body("Open the app to react.");
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

/// Load all recent positions with the DB with associated fcm tokens and send push
/// notification about expiring/expired positions if needed.
pub async fn query_and_send_position_notifications(
    conn: &mut PgConnection,
    notification_sender: &mpsc::Sender<Notification>,
) -> Result<()> {
    let positions_with_fcm_tokens = get_positions_joined_with_fcm_token_with_expiry_within(
        conn,
        OffsetDateTime::now_utc() - START_OF_EXPIRING_POSITION,
        OffsetDateTime::now_utc() + END_OF_EXPIRED_POSITION,
    )?;

    send_expiry_notifications_if_applicable(&positions_with_fcm_tokens, notification_sender).await;

    Ok(())
}

/// Send notifications to users with positions that are about to expire or have
/// just expired
async fn send_expiry_notifications_if_applicable(
    positions: &[(Position, FcmToken)],
    notification_sender: &mpsc::Sender<Notification>,
) {
    let now = OffsetDateTime::now_utc();

    for (position, fcm_token) in positions {
        if position.expiry_timestamp <= now
            && now < position.expiry_timestamp + END_OF_EXPIRED_POSITION
        {
            if let Err(e) = notification_sender
                .send(Notification::new(
                    fcm_token.clone(),
                    NotificationKind::PositionExpired,
                ))
                .await
            {
                tracing::error!("Failed to send PositionExpired notification: {:?}", e);
            }
        } else if position.expiry_timestamp > now + END_OF_EXPIRING_POSITION
            && position.expiry_timestamp <= now + START_OF_EXPIRING_POSITION
        {
            if let Err(e) = notification_sender
                .send(Notification::new(
                    fcm_token.clone(),
                    NotificationKind::PositionSoonToExpire,
                ))
                .await
            {
                tracing::error!("Failed to send PositionSoonToExpire notification: {:?}", e);
            }
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::logger::init_tracing_for_test;
    use time::Duration;

    fn soon_expiring_position() -> (Position, FcmToken) {
        let mut position = Position::dummy();
        position.creation_timestamp = OffsetDateTime::now_utc() - Duration::days(1);
        position.expiry_timestamp =
            OffsetDateTime::now_utc() + Duration::hours(12) + Duration::minutes(5);
        (position, FcmToken("soon_to_expire".to_string()))
    }

    /// This position is outside of the expiry notification window (12 hours
    /// before expiry), delivering notification for it would make little sense
    /// as user can't really react to it.
    fn just_before_expiry_position() -> (Position, FcmToken) {
        let mut position = Position::dummy();
        position.creation_timestamp = OffsetDateTime::now_utc() - Duration::days(1);
        position.expiry_timestamp = OffsetDateTime::now_utc() + Duration::hours(11);
        (position, FcmToken("soon_to_expire".to_string()))
    }

    fn just_expired_position() -> (Position, FcmToken) {
        let mut position = Position::dummy();
        position.creation_timestamp = OffsetDateTime::now_utc() - Duration::days(1);
        position.expiry_timestamp = OffsetDateTime::now_utc() - Duration::minutes(5);
        (position, FcmToken("just expired".to_string()))
    }

    fn ancient_expired_position() -> (Position, FcmToken) {
        let mut position = Position::dummy();
        position.creation_timestamp = OffsetDateTime::now_utc() - Duration::days(1);
        position.expiry_timestamp = OffsetDateTime::now_utc() - Duration::hours(2);
        (position, FcmToken("long time ago expired".to_string()))
    }

    fn far_from_expiry_position() -> (Position, FcmToken) {
        let mut position = Position::dummy();
        position.creation_timestamp = OffsetDateTime::now_utc() - Duration::days(1);
        position.expiry_timestamp = OffsetDateTime::now_utc() + Duration::days(1);
        (position, FcmToken("far_from_expiry".to_string()))
    }

    // Receive all values that could have been sent to the channel
    fn receive_all_notifications(
        notification_rx: &mut mpsc::Receiver<Notification>,
    ) -> Vec<Notification> {
        let mut received_notifications = vec![];
        while let Ok(notification) = notification_rx.try_recv() {
            tracing::info!(?notification, "Received notification");
            received_notifications.push(notification);
        }
        received_notifications
    }

    #[tokio::test]
    async fn send_no_notifications_when_too_far_to_expiry() {
        init_tracing_for_test();

        let position_far_from_expiring_1 = far_from_expiry_position();
        let position_far_from_expiring_2 = far_from_expiry_position();
        let (notification_sender, mut notification_receiver) = mpsc::channel(100);

        send_expiry_notifications_if_applicable(
            &[position_far_from_expiring_1, position_far_from_expiring_2],
            &notification_sender,
        )
        .await;

        let received_notifications = receive_all_notifications(&mut notification_receiver);

        assert_eq!(received_notifications.len(), 0);
    }

    #[tokio::test]
    async fn test_deliving_notifications_before_expiry() {
        init_tracing_for_test();

        let position_far_from_expiring = far_from_expiry_position();
        let position_soon_to_expire = soon_expiring_position();
        let position_just_before_expiry = just_before_expiry_position();
        let ancient_position = ancient_expired_position(); // too old to send notification
        let (notification_sender, mut notification_receiver) = mpsc::channel(100);

        send_expiry_notifications_if_applicable(
            &[
                position_far_from_expiring,
                position_just_before_expiry,
                position_soon_to_expire.clone(),
                ancient_position,
            ],
            &notification_sender,
        )
        .await;

        let received_notifications = receive_all_notifications(&mut notification_receiver);

        assert_eq!(received_notifications.len(), 1);

        let notification = received_notifications.first().unwrap();
        assert_eq!(
            notification.notification_kind,
            NotificationKind::PositionSoonToExpire
        );
        assert_eq!(notification.user_fcm_token, position_soon_to_expire.1);
    }

    #[tokio::test]
    async fn send_only_recently_expired_notifications() {
        init_tracing_for_test();

        let position_far_from_expiring = far_from_expiry_position();
        let position_just_expired = just_expired_position();
        let ancient_position = ancient_expired_position(); // too old to send notification
        let (notification_sender, mut notification_receiver) = mpsc::channel(100);

        send_expiry_notifications_if_applicable(
            &[
                position_far_from_expiring,
                position_just_expired.clone(),
                ancient_position,
            ],
            &notification_sender,
        )
        .await;

        let received_notifications = receive_all_notifications(&mut notification_receiver);

        assert_eq!(received_notifications.len(), 1);

        let notification = received_notifications.first().unwrap();
        assert_eq!(
            notification.notification_kind,
            NotificationKind::PositionExpired
        );
        assert_eq!(notification.user_fcm_token, position_just_expired.1);
    }
}
