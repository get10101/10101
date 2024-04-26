use crate::dlc::DlcChannel;
use crate::event;
use crate::event::EventInternal;
use crate::ln_dlc::node::Node;
use tokio::sync::broadcast::error::RecvError;
use xxi_node::node::event::NodeEvent;

impl Node {
    pub fn spawn_listen_dlc_channels_event_task(&self) {
        let mut receiver = self.inner.event_handler.subscribe();

        tokio::spawn({
            let node = self.clone();
            async move {
                loop {
                    match receiver.recv().await {
                        Ok(NodeEvent::DlcChannelEvent { dlc_channel_event }) => {
                            if let Some(reference_id) = dlc_channel_event.get_reference_id() {
                                match node.inner.get_dlc_channel_by_reference_id(reference_id) {
                                    Ok(channel) => event::publish(&EventInternal::DlcChannelEvent(
                                        DlcChannel::from(&channel),
                                    )),
                                    Err(e) => tracing::error!(
                                        ?reference_id,
                                        "Failed to get dlc channel by reference id. Error: {e:#}"
                                    ),
                                }
                            }
                        }
                        Ok(NodeEvent::Connected { .. })
                        | Ok(NodeEvent::SendDlcMessage { .. })
                        | Ok(NodeEvent::StoreDlcMessage { .. })
                        | Ok(NodeEvent::SendLastDlcMessage { .. }) => {} // ignored
                        Err(RecvError::Lagged(skipped)) => {
                            tracing::warn!("Skipped {skipped} messages");
                        }
                        Err(RecvError::Closed) => {
                            tracing::error!("Lost connection to sender!");
                            break;
                        }
                    }
                }
            }
        });
    }
}
