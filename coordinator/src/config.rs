use lightning::ln::channelmanager::MIN_CLTV_EXPIRY_DELTA;
use lightning::util::config::ChannelConfig;
use lightning::util::config::ChannelHandshakeConfig;
use lightning::util::config::ChannelHandshakeLimits;
use lightning::util::config::UserConfig;

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
        // The coordinator automatically accepts any inbound channels if they adhere to its channel
        // preferences.
        manually_accept_inbound_channels: false,
        ..Default::default()
    }
}
