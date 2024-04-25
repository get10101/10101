use crate::message_handler::TenTenOneMessage;
use bitcoin::secp256k1::PublicKey;
use ln_dlc_storage::DlcChannelEvent;
use std::sync::mpsc;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::broadcast::Receiver;
use tokio::task::spawn_blocking;

#[derive(Clone, Debug)]
pub enum NodeEvent {
    Connected {
        peer: PublicKey,
    },
    SendDlcMessage {
        peer: PublicKey,
        msg: TenTenOneMessage,
    },
    StoreDlcMessage {
        peer: PublicKey,
        msg: TenTenOneMessage,
    },
    SendLastDlcMessage {
        peer: PublicKey,
    },
    DlcChannelEvent {
        dlc_channel_event: DlcChannelEvent,
    },
}

#[derive(Clone)]
pub struct NodeEventHandler {
    sender: broadcast::Sender<NodeEvent>,
}

impl Default for NodeEventHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeEventHandler {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(100);

        NodeEventHandler { sender }
    }

    pub fn subscribe(&self) -> Receiver<NodeEvent> {
        self.sender.subscribe()
    }

    pub fn publish(&self, event: NodeEvent) {
        if let Err(e) = self.sender.send(event) {
            tracing::error!("Failed to send node event. Error {e:#}");
        }
    }
}

pub fn connect_node_event_handler_to_dlc_channel_events(
    node_event_handler: Arc<NodeEventHandler>,
    dlc_event_receiver: mpsc::Receiver<DlcChannelEvent>,
) {
    spawn_blocking(move || loop {
        match dlc_event_receiver.recv() {
            Ok(dlc_channel_event) => {
                node_event_handler.publish(NodeEvent::DlcChannelEvent { dlc_channel_event })
            }
            Err(e) => {
                tracing::error!("The dlc event channel has been closed. Error: {e:#}");
                break;
            }
        }
    });
}
