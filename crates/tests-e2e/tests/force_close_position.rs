#![allow(clippy::unwrap_used)]

use native::api::ChannelState;
use native::api::SignedChannelState;
use tests_e2e::app::force_close_dlc_channel;
use tests_e2e::app::get_dlc_channels;
use tests_e2e::setup;
use tests_e2e::wait_until;

#[tokio::test(flavor = "multi_thread")]
#[ignore = "need to be run with 'just e2e' command"]
async fn can_force_close_position() {
    setup::TestSetup::new_with_open_position().await;

    force_close_dlc_channel();

    let channels = get_dlc_channels();
    let channel = channels.first().unwrap();

    wait_until!(matches!(
        channel.channel_state,
        ChannelState::Signed {
            state: SignedChannelState::Closing { .. },
            ..
        }
    ));

    // TODO: Assert that the position is closed in the app and that the DLC is claimed correctly
    // on-chain.
}
