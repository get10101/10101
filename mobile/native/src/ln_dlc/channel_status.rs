use crate::event;
use crate::ln_dlc::node::Node;
use anyhow::Result;
use ln_dlc_node::node::rust_dlc_manager::channel::signed_channel::SignedChannel;
use ln_dlc_node::node::rust_dlc_manager::channel::signed_channel::SignedChannelState;
use ln_dlc_node::node::rust_dlc_manager::subchannel::SubChannel;
use std::borrow::Borrow;
use std::time::Duration;

const UPDATE_CHANNEL_STATUS_INTERVAL: Duration = Duration::from_secs(5);

/// The status of the app channel
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelStatus {
    /// No channel is open.
    ///
    /// This means that it is possible to open a new DLC channel. This does _not_ indicate if
    /// there was a previous channel nor does it imply that a previous channel was completely
    /// closed i.e. there might be pending transactions.
    NotOpen,
    /// DLC Channel is open
    Open,
    /// DLC Channel is open and has an open position
    WithPosition,
    /// DLC Channel is currently in progress of being renewed
    Pending,
    /// DLC Channel is currently in progress of being closed
    Closing,
    /// The status of the channel is not known.
    Unknown,
}

pub async fn track_channel_status(node: impl Borrow<Node>) {
    let mut cached_status = ChannelStatus::Unknown;
    loop {
        tracing::trace!("Tracking channel status");

        let status = channel_status(node.borrow())
            .await
            .map_err(|e| {
                tracing::error!("Could not compute LN-DLC channel status: {e:#}");
                e
            })
            .into();

        if status != cached_status {
            tracing::info!(?status, "Channel status update");
            event::publish(&event::EventInternal::ChannelStatusUpdate(status));
            cached_status = status;
        }

        tokio::time::sleep(UPDATE_CHANNEL_STATUS_INTERVAL).await;
    }
}

impl From<Result<ConcreteChannelStatus>> for ChannelStatus {
    fn from(value: Result<ConcreteChannelStatus>) -> Self {
        match value {
            Ok(ConcreteChannelStatus::NotOpen) => Self::NotOpen,
            Ok(ConcreteChannelStatus::Open) => Self::Open,
            Ok(ConcreteChannelStatus::Pending) => Self::Pending,
            Ok(ConcreteChannelStatus::WithPosition) => Self::WithPosition,
            Ok(ConcreteChannelStatus::Closing) => Self::Closing,
            Err(_) => Self::Unknown,
        }
    }
}

/// Figure out the status of the current channel.
async fn channel_status(node: impl Borrow<Node>) -> Result<ConcreteChannelStatus> {
    let node: &Node = node.borrow();
    let node = &node.inner;

    let dlc_channels = node.list_dlc_channels()?;
    if dlc_channels.len() > 1 {
        tracing::warn!(
            channels = dlc_channels.len(),
            "We have more than one DLC channel. This should not happen"
        );
    }

    let maybe_dlc_channel = dlc_channels.first();

    let status = derive_dlc_channel_status(maybe_dlc_channel);

    Ok(status)
}

/// The concrete status of the app's channel.
///
/// By concrete we mean that the channel status can be determined because we have all the data
/// needed to derive it.
#[derive(Debug, Clone, Copy)]
enum ConcreteChannelStatus {
    NotOpen,
    Pending,
    Open,
    WithPosition,
    Closing,
}

fn derive_dlc_channel_status(dlc_channel: Option<&SignedChannel>) -> ConcreteChannelStatus {
    match dlc_channel {
        Some(SignedChannel {
            state: SignedChannelState::SettledConfirmed { .. },
            ..
        })
        | Some(SignedChannel {
            state: SignedChannelState::SettledOffered { .. },
            ..
        })
        | Some(SignedChannel {
            state: SignedChannelState::SettledReceived { .. },
            ..
        })
        | Some(SignedChannel {
            state: SignedChannelState::SettledAccepted { .. },
            ..
        })
        | Some(SignedChannel {
            state: SignedChannelState::RenewOffered { .. },
            ..
        })
        | Some(SignedChannel {
            state: SignedChannelState::RenewAccepted { .. },
            ..
        })
        | Some(SignedChannel {
            state: SignedChannelState::RenewConfirmed { .. },
            ..
        }) => ConcreteChannelStatus::Pending,
        Some(SignedChannel {
            state: SignedChannelState::Closing { .. },
            ..
        })
        | Some(SignedChannel {
            state: SignedChannelState::CollaborativeCloseOffered { .. },
            ..
        }) => ConcreteChannelStatus::Closing,
        Some(SignedChannel {
            state: SignedChannelState::Established { .. },
            ..
        })
        | Some(SignedChannel {
            state: SignedChannelState::RenewFinalized { .. },
            ..
        }) => ConcreteChannelStatus::WithPosition,
        Some(SignedChannel {
            state: SignedChannelState::Settled { .. },
            ..
        }) => ConcreteChannelStatus::Open,
        None => ConcreteChannelStatus::NotOpen,
    }
}

enum SubChannelState {
    Rejected,
    Opening,
    Open,
    CollabClosing,
    CollabClosed,
    ForceClosing,
    ForceClosed,
}

impl From<&SubChannel> for SubChannelState {
    fn from(value: &SubChannel) -> Self {
        use ln_dlc_node::node::rust_dlc_manager::subchannel::SubChannelState::*;
        match value.state {
            Rejected => SubChannelState::Rejected,
            Offered(_) | Accepted(_) | Confirmed(_) | Finalized(_) => SubChannelState::Opening,
            Signed(_) => SubChannelState::Open,
            CloseOffered(_) | CloseAccepted(_) | CloseConfirmed(_) => {
                SubChannelState::CollabClosing
            }
            OffChainClosed => SubChannelState::CollabClosed,
            Closing(_) => SubChannelState::ForceClosing,
            OnChainClosed | CounterOnChainClosed | ClosedPunished(_) => {
                SubChannelState::ForceClosed
            }
        }
    }
}
