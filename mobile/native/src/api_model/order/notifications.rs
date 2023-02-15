use crate::api_model::order::OrderNotification;
use anyhow::anyhow;
use anyhow::Result;
use flutter_rust_bridge::support::lazy_static;
use flutter_rust_bridge::StreamSink;
pub use std::sync::Mutex;

lazy_static! {
    static ref ORDER_NOTIFICATION_STREAM_SINK: Mutex<Option<StreamSink<OrderNotification>>> =
        Default::default();
}

pub fn add_listener(listener: StreamSink<OrderNotification>) -> Result<()> {
    match ORDER_NOTIFICATION_STREAM_SINK.lock() {
        Ok(mut guard) => {
            *guard = Some(listener);
            Ok(())
        }
        Err(err) => Err(anyhow!("Could not register event listener: {}", err)),
    }
}

pub fn send_notification(order_notification: OrderNotification) {
    if let Ok(mut guard) = ORDER_NOTIFICATION_STREAM_SINK.lock() {
        if let Some(sink) = guard.as_mut() {
            sink.add(order_notification);
        }
    }
}
