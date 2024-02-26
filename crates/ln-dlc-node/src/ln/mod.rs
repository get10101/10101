use crate::GossipSync;
use crate::P2pGossipSync;
use crate::RapidGossipSync;
use std::sync::Arc;

mod app_event_handler;
mod contract_details;
mod coordinator_event_handler;
mod dlc_channel_details;
mod event_handler;
mod logger;
mod manage_spendable_outputs;

/// A collection of handlers for events emitted by the Lightning node.
///
/// When constructing a new [`Node`], you can pass in a custom [`EventHandler`]
/// to handle events; these handlers are useful to reduce boilerplate if you
/// don't require custom behaviour.
pub mod common_handlers;

pub use app_event_handler::AppEventHandler;
pub use contract_details::ContractDetails;
pub use coordinator_event_handler::CoordinatorEventHandler;
pub use dlc_channel_details::DlcChannelDetails;
pub use event_handler::EventHandlerTrait;
pub use event_handler::EventSender;
pub(crate) use logger::TracingLogger;
pub(crate) use manage_spendable_outputs::manage_spendable_outputs;

#[derive(Clone)]
pub enum GossipSource {
    P2pNetwork {
        gossip_sync: Arc<P2pGossipSync>,
    },
    RapidGossipSync {
        gossip_sync: Arc<RapidGossipSync>,
        server_url: String,
    },
}

impl GossipSource {
    pub fn as_gossip_sync(&self) -> GossipSync {
        match self {
            Self::RapidGossipSync { gossip_sync, .. } => GossipSync::Rapid(gossip_sync.clone()),
            Self::P2pNetwork { gossip_sync } => GossipSync::P2P(gossip_sync.clone()),
        }
    }
}
