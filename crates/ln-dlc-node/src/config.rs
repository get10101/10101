use lightning::chain::chaininterface::ConfirmationTarget;
use lightning::ln::channelmanager::MIN_CLTV_EXPIRY_DELTA;
use lightning::util::config::ChannelConfig;
use lightning::util::config::ChannelHandshakeConfig;
use lightning::util::config::ChannelHandshakeLimits;
use lightning::util::config::MaxDustHTLCExposure;
use lightning::util::config::UserConfig;
use std::time::Duration;

/// The speed at which we want a transaction to confirm used for feerate estimation.
///
/// We set it to high priority because the channel funding transaction should be included fast.
pub const CONFIRMATION_TARGET: ConfirmationTarget = ConfirmationTarget::HighPriority;

/// When handling the [`Event::HTLCIntercepted`], the user might not be online right away. This
/// could be because she is funding the wallet through another wallet. In order to give the user
/// some time to open 10101 again we wait for a bit to see if we can establish a connection.
///
/// This constant specifies the amount of time we are willing to delay a payment.
pub(crate) const HTLC_INTERCEPTED_CONNECTION_TIMEOUT: Duration = Duration::from_secs(30);

pub fn app_config() -> UserConfig {
    UserConfig {
        channel_handshake_config: ChannelHandshakeConfig {
            // The app will only accept private channels. As we are forcing the apps announced
            // channel preferences, the coordinator needs to override this config to match the apps
            // preferences.
            announced_channel: false,
            minimum_depth: 1,
            // There is no risk in the leaf channel to receive 100% of the channel capacity.
            max_inbound_htlc_value_in_flight_percent_of_channel: 100,
            // We want the coordinator to recover force-close funds as soon as possible. We choose
            // 144 because we can't go any lower according to LDK.
            our_to_self_delay: 144,
            // Setting this to 0 will default to 1000 sats. Meaning, that the coordinator only have
            // to keep 1000 sats on reserve of the ln channel.
            their_channel_reserve_proportional_millionths: 0,
            ..Default::default()
        },
        channel_handshake_limits: ChannelHandshakeLimits {
            max_minimum_depth: 1,
            trust_own_funding_0conf: true,
            // Enforces that incoming channels will be private.
            force_announced_channel_preference: true,
            // We want app users to only have to wait ~24 hours in case of a force-close. We choose
            // 144 because we can't go any lower according to LDK.
            their_to_self_delay: 144,
            max_funding_satoshis: 100_000_000,
            ..Default::default()
        },
        channel_config: ChannelConfig {
            cltv_expiry_delta: MIN_CLTV_EXPIRY_DELTA,
            // Allows the coordinator to charge us a channel-opening fee after intercepting the
            // app's funding HTLC.
            accept_underpaying_htlcs: true,
            // Setting this to the maximum value to ensure that a payment will not fail because of
            // dust exposure due to high on-chain fees.
            max_dust_htlc_exposure: MaxDustHTLCExposure::FixedLimitMsat(
                21_000_000 * 100_000_000 * 1_000,
            ),
            ..Default::default()
        },
        // we want to accept 0-conf channels from the coordinator
        manually_accept_inbound_channels: true,
        ..Default::default()
    }
}

pub fn coordinator_config() -> UserConfig {
    UserConfig {
        channel_handshake_config: ChannelHandshakeConfig {
            // The coordinator will by default only accept public channels (see also
            // `force_announced_channel_preference`). In order to open a private channel with the
            // mobile app this config gets overwritten during the creation of the just-in-time
            // channel. Note, public channels need 6 confirmations to get announced (and usable for
            // multi-hop payments). This is a requirement of BOLT 7.
            announced_channel: true,
            // The minimum amount of confirmations before the inbound channel is deemed usable,
            // between the counterparties.
            minimum_depth: 1,
            // We set this 100% as the coordinator is online 24/7 and can take the risk.
            max_inbound_htlc_value_in_flight_percent_of_channel: 100,
            // Our channel peers are allowed to get back their funds ~24 hours after a
            // force-closure.
            our_to_self_delay: 144,
            // enable anchor output support on the coordinator with public channels. Not used with
            // the app.
            negotiate_anchors_zero_fee_htlc_tx: true,
            ..Default::default()
        },
        channel_handshake_limits: ChannelHandshakeLimits {
            // The minimum amount of confirmations before the outbound channel is deemed usable,
            // between the counterparties.
            max_minimum_depth: 3,
            trust_own_funding_0conf: true,
            // Enforces incoming channels to the coordinator to be public! We
            // only want to open private channels to our 10101 app.
            force_announced_channel_preference: true,
            // LND's max to_self_delay is 2016, so we want to be compatible.
            their_to_self_delay: 2016,
            max_funding_satoshis: 500_000_000,
            ..Default::default()
        },
        channel_config: ChannelConfig {
            cltv_expiry_delta: MIN_CLTV_EXPIRY_DELTA,
            // Proportional fee charged for forwarding a payment (outbound through a channel of
            // ours).
            forwarding_fee_proportional_millionths: 50,
            // A base fee of 0 is chosen to simplify path-finding.
            forwarding_fee_base_msat: 0,
            ..Default::default()
        },
        // This is needed to intercept payments to open just-in-time channels. This will produce the
        // HTLCIntercepted event.
        accept_intercept_htlcs: true,
        // This config is needed to forward payments to the 10101 app, which only have private
        // channels with the coordinator.
        accept_forwards_to_priv_channels: true,
        // By enabling anchor outputs, we have to manually check if the provided reserve is
        // sufficient.
        manually_accept_inbound_channels: true,
        ..Default::default()
    }
}
