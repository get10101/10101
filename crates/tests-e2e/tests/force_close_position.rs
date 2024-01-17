#![allow(clippy::unwrap_used)]

use native::ln_dlc::ChannelStatus;
use tests_e2e::app::force_close_dlc_channel;
use tests_e2e::setup;
use tests_e2e::wait_until;

#[tokio::test(flavor = "multi_thread")]
#[ignore = "need to be run with 'just e2e' command"]
async fn can_force_close_position() {
    let test = setup::TestSetup::new_with_open_position().await;

    force_close_dlc_channel();

    wait_until!(test.app.rx.channel_status() == Some(ChannelStatus::Closing));

    // TODO: Assert that the position is closed in the app and that the DLC is claimed correctly
    // on-chain.
}
