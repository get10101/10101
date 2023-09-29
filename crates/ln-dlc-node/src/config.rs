use lightning::chain::chaininterface::ConfirmationTarget;
use std::time::Duration;

/// The multiplier to be used by the coordinator to define the just in time channel liquidity
///
/// The liquidity provided by the trader will be multiplied with this value to defined the channel
/// value.
/// See `JUST_IN_TIME_CHANNEL_OUTBOUND_LIQUIDITY_SAT_MAX` for the maximum channel value.
pub const LIQUIDITY_MULTIPLIER: u64 = 2;

/// The speed at which we want a transaction to confirm used for feerate estimation.
///
/// We set it to high priority because the channel funding transaction should be included fast.
pub const CONFIRMATION_TARGET: ConfirmationTarget = ConfirmationTarget::HighPriority;

/// When handling the [`Event::HTLCIntercepted`], the user might not be online right away. This
/// could be because she is funding the wallet through another wallet. In order to give the user
/// some time to open 10101 again we wait for a bit to see if we can establish a connection.
///
/// This constant specifies the amount of time we are willing to delay a payment.
pub const HTLC_INTERCEPTED_CONNECTION_TIMEOUT: Duration = Duration::from_secs(30);
