use flutter_rust_bridge::frb;

#[frb]
#[derive(Debug, Clone, Copy)]
pub enum PositionNotificationType {
    New,
    Update,
}

#[frb]
#[derive(Debug, Clone)]
pub struct PositionNotification {
    pub id: String,
    pub notification_type: PositionNotificationType,
}
