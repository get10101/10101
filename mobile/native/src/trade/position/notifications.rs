use crate::api_model::position::PositionNotification;
use anyhow::anyhow;
use anyhow::Result;
use flutter_rust_bridge::support::lazy_static;
use flutter_rust_bridge::StreamSink;
pub use std::sync::Mutex;

lazy_static! {
    static ref POSITION_NOTIFICATION_STREAM_SINK: Mutex<Option<StreamSink<PositionNotification>>> =
        Default::default();
}

pub fn add_listener(listener: StreamSink<PositionNotification>) -> Result<()> {
    match POSITION_NOTIFICATION_STREAM_SINK.lock() {
        Ok(mut guard) => {
            *guard = Some(listener);
            Ok(())
        }
        Err(err) => Err(anyhow!("Could not register event listener: {}", err)),
    }
}

pub fn send_notification(position_notification: PositionNotification) {
    if let Ok(mut guard) = POSITION_NOTIFICATION_STREAM_SINK.lock() {
        if let Some(sink) = guard.as_mut() {
            sink.add(position_notification);
        }
    }
}
