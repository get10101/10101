mod channel_details;
mod config;
mod dlc_channel_details;
mod event_handler;
mod logger;

pub use channel_details::ChannelDetails;
pub(crate) use config::app_config;
pub(crate) use config::coordinator_config;
pub use dlc_channel_details::DlcChannelDetails;
pub(crate) use event_handler::EventHandler;
pub(crate) use logger::TracingLogger;

/// When handling the [`Event::HTLCIntercepted`], we may need to
/// create a new channel with the recipient of the HTLC. If the
/// payment is small enough (< 1000 sats), opening the channel will
/// fail unless we provide more outbound liquidity.
///
/// This value is completely arbitrary at this stage. Eventually, we
/// should, for example, let the payee decide how much inbound
/// liquidity they desire, and charge them for it.
///
/// This constant only applies to the coordinator.
pub(crate) const JUST_IN_TIME_CHANNEL_OUTBOUND_LIQUIDITY_SAT: u64 = 200_000;

/// When handling the [`Event::HTLCIntercepted`], the user might not be online right away. This
/// could be because she is funding the wallet through another wallet. In order to give the user
/// some time to open 10101 again we wait for a bit to see if we can establish a connection.
///
/// This constant specifies the amount of time (in seconds) we are willing to delay a payment.
pub(crate) const HTLC_INTERCEPTED_CONNECTION_TIMEOUT: u64 = 30;

/// The delay before sending the node announcement after the [`Event::ChannelReady`] event.
pub(crate) const BROADCAST_NODE_ANNOUNCEMENT_DELAY: u64 = 10;
