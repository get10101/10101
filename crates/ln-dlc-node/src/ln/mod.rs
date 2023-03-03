mod config;
mod event_handler;
mod logger;

pub(crate) use config::app_config;
pub(crate) use config::coordinator_config;
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
pub(crate) const JUST_IN_TIME_CHANNEL_OUTBOUND_LIQUIDITY_SAT: u64 = 10_000;
