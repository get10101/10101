use ln_dlc_node::lightning::ln::channelmanager::MIN_CLTV_EXPIRY_DELTA;
use ln_dlc_node::lightning::util::config::ChannelConfig;
use ln_dlc_node::lightning::util::config::ChannelHandshakeConfig;
use ln_dlc_node::lightning::util::config::ChannelHandshakeLimits;
use ln_dlc_node::lightning::util::config::UserConfig;

mod event_handler;

pub use event_handler::EventHandler;

pub fn ldk_config() -> UserConfig {
    UserConfig {
        channel_handshake_config: ChannelHandshakeConfig {
            // The coordinator mandates this.
            announced_channel: true,
            minimum_depth: 1,
            // There is no risk in leaf channels receiving 100% of the channel capacity.
            max_inbound_htlc_value_in_flight_percent_of_channel: 100,
            // We want the coordinator to recover force-close funds as soon as possible. We choose
            // 144 because we can't go any lower according to LDK.
            our_to_self_delay: 144,
            ..Default::default()
        },
        channel_handshake_limits: ChannelHandshakeLimits {
            max_minimum_depth: 1,
            // We want makers to only have to wait ~24 hours in case of a force-close. We choose 144
            // because we can't go any lower according to LDK.
            their_to_self_delay: 144,
            max_funding_satoshis: 100_000_000,
            ..Default::default()
        },
        channel_config: ChannelConfig {
            cltv_expiry_delta: MIN_CLTV_EXPIRY_DELTA,
            ..Default::default()
        },
        ..Default::default()
    }
}
