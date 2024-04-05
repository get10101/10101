use crate::db;
use crate::dlc::ChannelState;
use crate::dlc::DlcChannel;
use crate::dlc::SignedChannelState;
use crate::event;
use crate::event::subscriber::Subscriber;
use crate::event::EventInternal;
use crate::event::EventType;
use trade::ContractSymbol;

#[derive(Clone, Copy)]
pub struct ForceCloseDlcChannelSubscriber;

impl Subscriber for ForceCloseDlcChannelSubscriber {
    fn notify(&self, event: &EventInternal) {
        let runtime = match crate::state::get_or_create_tokio_runtime() {
            Ok(runtime) => runtime,
            Err(e) => {
                tracing::error!("Failed to get tokio runtime. Error: {e:#}");
                return;
            }
        };
        runtime.spawn_blocking({
            let event = event.clone();
            move || {
                if matches!(
                    event,
                    EventInternal::DlcChannelEvent(DlcChannel {
                        channel_state: ChannelState::Signed {
                            state: SignedChannelState::Closing,
                            ..
                        },
                        ..
                    })
                ) {
                    tracing::warn!("Removing position after dlc channel got force closed.");
                    if let Err(e) = db::delete_positions() {
                        tracing::error!("Failed to delete position after the dlc channel has been force closed. Error: {e:#}")
                    }
                    event::publish(&EventInternal::PositionCloseNotification(
                        ContractSymbol::BtcUsd,
                    ));
                }}
        });
    }

    fn events(&self) -> Vec<EventType> {
        vec![EventType::DlcChannelEvent]
    }
}
