use crate::db::get_all_non_pending_channels;
use crate::event;
use crate::ln_dlc::node::Node;
use anyhow::Result;
use ln_dlc_node::channel::Channel;
use ln_dlc_node::channel::ChannelState;
use ln_dlc_node::node::rust_dlc_manager::subchannel::SubChannel;
use std::borrow::Borrow;
use std::time::Duration;

const UPDATE_CHANNEL_STATUS_INTERVAL: Duration = Duration::from_secs(5);

/// The status of the app channel
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelStatus {
    /// No channel is open.
    ///
    /// This means that it is possible to open a new Lightning channel. This does _not_ indicate if
    /// there was a previous channel nor does it imply that a previous channel was completely
    /// closed i.e. there might be pending transactions.
    NotOpen,
    /// There is a vanilla Lightning channel, without a subchannel.1
    ///
    /// This means that it is possible to use the Lightning channel for payments and that the
    /// channel can be upgraded to an LN-DLC channel.
    LnOpen,
    /// There is an LN-DLC channel.
    ///
    /// This corresponds to an app which currently has an open position as a result of trading.
    LnDlcOpen,
    /// The LN-DLC channel is in the process of being force-closed.
    LnDlcForceClosing,
    /// The LN-DLC channel is in an inconsistent state.
    Inconsistent,
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
            Ok(ConcreteChannelStatus::LnOpen) => Self::LnOpen,
            Ok(ConcreteChannelStatus::LnDlcOpen) => Self::LnDlcOpen,
            Ok(ConcreteChannelStatus::LnDlcForceClosing) => Self::LnDlcForceClosing,
            Ok(ConcreteChannelStatus::Inconsistent) => Self::Inconsistent,
            Err(_) => Self::Unknown,
        }
    }
}

/// Figure out the status of the current channel.
async fn channel_status(node: impl Borrow<Node>) -> Result<ConcreteChannelStatus> {
    let node: &Node = node.borrow();
    let node = &node.inner;

    // We assume that the most recently created LN channel is the current one. We only care about
    // that one because the app can only have one channel at a time.
    let ln_channel = get_all_non_pending_channels()?
        .iter()
        .max_by(|a, b| a.created_at.cmp(&b.created_at))
        .cloned();

    let ln_channel = match ln_channel {
        Some(ln_channel) => ln_channel,
        // We never even had one LN channel.
        None => return Ok(ConcreteChannelStatus::NotOpen),
    };

    let subchannels = node.list_sub_channels()?;

    let status = derive_ln_dlc_channel_status(ln_channel, subchannels);

    Ok(status)
}

/// The concrete status of the app channels.
///
/// By concrete we mean that the channel status can be determined because we have all the data
/// needed to derive it.
#[derive(Debug, Clone, Copy)]
enum ConcreteChannelStatus {
    NotOpen,
    LnOpen,
    LnDlcOpen,
    // We cannot easily model the vanilla Lightning channel being force-closed because LDK erases
    // a closed channel from the `ChannelManager` as soon as the closing process begins. This is a
    // problem because it's not safe to delete the app if the channel is not fully closed!
    //
    // TODO: Add support for `LnForceClosing` status and ensure that channel status remains in
    // `LnDlcForceClosing` whilst the LN channel is still closing.
    // LnForceClosing,
    LnDlcForceClosing,
    Inconsistent,
}

// TODO: We currently only look at the state of the subchannel. We should also take into account the
// state of the DLC channel and the contract. Otherwise we won't be able to convey that the CET is
// not yet confirmed.
//
// TODO: We should map the argument types to our own types so that we can simplify them. This will
// make it so much easier to build test cases in order to add tests.
fn derive_ln_dlc_channel_status(
    // The most recently created LN channel, whether open or not.
    ln_channel: Channel,
    // All the subchannels that we have ever recorded.
    subchannels: Vec<SubChannel>,
) -> ConcreteChannelStatus {
    match ln_channel {
        // If the LN channel is open (or practically open).
        Channel {
            channel_id: Some(channel_id),
            channel_state: ChannelState::Pending | ChannelState::Open | ChannelState::OpenUnpaid,
            ..
        } => {
            // We might have more than one subchannel stored, but we only care about the one that
            // corresponds to the open LN channel.
            match subchannels
                .iter()
                .find(|subchannel| subchannel.channel_id == channel_id)
            {
                None => ConcreteChannelStatus::LnOpen,
                Some(subchannel) => match SubChannelState::from(subchannel) {
                    SubChannelState::Rejected
                    | SubChannelState::Opening
                    | SubChannelState::CollabClosed => ConcreteChannelStatus::LnOpen,
                    SubChannelState::Open | SubChannelState::CollabClosing => {
                        ConcreteChannelStatus::LnDlcOpen
                    }
                    // We're still waiting for the LN channel to close.
                    SubChannelState::ForceClosing | SubChannelState::ForceClosed => {
                        ConcreteChannelStatus::LnDlcForceClosing
                    }
                },
            }
        }
        // If the LN channel is closing or closed. To discern between the two we would need to know
        // the status of the commitment transaction.
        Channel {
            channel_id: Some(channel_id),
            channel_state:
                ChannelState::Closed | ChannelState::ForceClosedLocal | ChannelState::ForceClosedRemote,
            ..
        } => {
            match subchannels
                .iter()
                .find(|subchannel| subchannel.channel_id == channel_id)
            {
                // We never had a subchannel associated with the latest LN channel.
                None => ConcreteChannelStatus::NotOpen,
                Some(subchannel) => match SubChannelState::from(subchannel) {
                    // The subchannel was never fully open.
                    SubChannelState::Rejected | SubChannelState::Opening => {
                        ConcreteChannelStatus::NotOpen
                    }
                    // The subchannel was closed offchain.
                    SubChannelState::CollabClosed => ConcreteChannelStatus::NotOpen,
                    // The subchannel was close on-chain.
                    SubChannelState::ForceClosed => ConcreteChannelStatus::NotOpen,
                    // The subchannel is somehow still open even though the LN channel is not.
                    SubChannelState::Open | SubChannelState::CollabClosing => {
                        ConcreteChannelStatus::Inconsistent
                    }
                    // The subchannel is still open being closed on-chain.
                    SubChannelState::ForceClosing => ConcreteChannelStatus::LnDlcForceClosing,
                },
            }
        }
        // If the LN channel does not have an ID associated with it, we assume that it's still not
        // open, for it should not be usable yet.
        Channel {
            channel_id: None, ..
        } => ConcreteChannelStatus::NotOpen,
        Channel {
            channel_state: ChannelState::Announced,
            ..
        } => unimplemented!("This state does not apply to the app"),
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
