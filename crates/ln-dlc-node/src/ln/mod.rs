mod channel_details;
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

pub use channel_details::ChannelDetails;
pub use dlc_channel_details::DlcChannelDetails;
pub use event_handler::EventHandlerTrait;
pub use event_handler::EventSender;
pub use event_handler::InterceptionDetails;
pub use event_handler::PendingInterceptedHtlcs;
pub(crate) use logger::TracingLogger;
pub(crate) use manage_spendable_outputs::manage_spendable_outputs;
