use lightning::ln::channelmanager::MIN_CLTV_EXPIRY_DELTA;
use lightning::util::config::ChannelConfig;
use lightning::util::config::ChannelHandshakeConfig;
use lightning::util::config::ChannelHandshakeLimits;
use lightning::util::config::UserConfig;

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
            ..Default::default()
        },
        channel_handshake_limits: ChannelHandshakeLimits {
            max_minimum_depth: 1,
            trust_own_funding_0conf: true,
            // Enforces that incoming channels will be private.
            force_announced_channel_preference: true,
            // lnd's max to_self_delay is 2016, so we want to be compatible.
            their_to_self_delay: 2016,
            ..Default::default()
        },
        channel_config: ChannelConfig {
            cltv_expiry_delta: MIN_CLTV_EXPIRY_DELTA,
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
            // The coordinator will by default only accept public channels. (see also
            // force_announced_channel_preference). In order to open a private channel with the
            // mobile app this config gets overwritten during the creation of the just-in-time
            // channel)
            // Note, public channels need 6 confirmations to get announced (and usable for multi-hop
            // payments) this is a requirement of BOLT 7.
            announced_channel: true,
            // The minimum amount of confirmations before the inbound channel is deemed useable,
            // between the counterparties
            minimum_depth: 1,
            // We set this 100% as the coordinator is online 24/7 and can take the risk.
            max_inbound_htlc_value_in_flight_percent_of_channel: 100,
            ..Default::default()
        },
        channel_handshake_limits: ChannelHandshakeLimits {
            // The minimum amount of confirmations before the outbound channel is deemed useable,
            // between the counterparties
            max_minimum_depth: 1,
            trust_own_funding_0conf: true,
            // Enforces incoming channels to the coordinator to be public! We
            // only want to open private channels to our 10101 app.
            force_announced_channel_preference: true,
            // lnd's max to_self_delay is 2016, so we want to be compatible.
            their_to_self_delay: 2016,
            ..Default::default()
        },
        channel_config: ChannelConfig {
            cltv_expiry_delta: MIN_CLTV_EXPIRY_DELTA,
            ..Default::default()
        },
        // This is needed to intercept payments to open just-in-time channels. This will produce the
        // HTLCIntercepted event.
        accept_intercept_htlcs: true,
        // This config is needed to forward payments to the 10101 app, which only have private
        // channels with the coordinator.
        accept_forwards_to_priv_channels: true,
        // the coordinator automatically accepts any inbound channels if the adhere to it's channel
        // preferences. (public, etc.)
        manually_accept_inbound_channels: false,
        ..Default::default()
    }
}
