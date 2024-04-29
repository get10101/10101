#![allow(clippy::unwrap_used)]

use bitcoin::Network;
use native::api;
use native::api::ChannelState;
use native::api::SignedChannelState;
use native::trade::position;
use position::PositionState;
use tests_e2e::app::force_close_dlc_channel;
use tests_e2e::app::get_dlc_channels;
use tests_e2e::app::AppHandle;
use tests_e2e::setup;
use tests_e2e::wait_until;
use time::OffsetDateTime;
use xxi_node::commons;

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn can_rollover_position() {
    let test = setup::TestSetup::new_with_open_position().await;
    let coordinator = &test.coordinator;
    let dlc_channels = coordinator.get_dlc_channels().await.unwrap();
    let app_pubkey = api::get_node_id().0;

    tracing::info!("{:?}", dlc_channels);

    let dlc_channel = dlc_channels
        .into_iter()
        .find(|chan| chan.counter_party == app_pubkey)
        .unwrap();

    let new_expiry = commons::calculate_next_expiry(OffsetDateTime::now_utc(), Network::Regtest);

    coordinator
        .rollover(&dlc_channel.dlc_channel_id.unwrap())
        .await
        .unwrap();

    wait_until!(check_rollover_position(&test.app, new_expiry));
    wait_until!(test
        .app
        .rx
        .position()
        .map(|p| PositionState::Open == p.position_state)
        .unwrap_or(false));

    // Once the rollover is complete, we also want to verify that the channel can still be
    // force-closed. This should be tested in `rust-dlc`, but we recently encountered a bug in our
    // branch: https://github.com/get10101/10101/pull/2079.

    force_close_dlc_channel(&test.bitcoind).await;

    let channels = get_dlc_channels();
    let channel = channels.first().unwrap();

    wait_until!(matches!(
        channel.channel_state,
        ChannelState::Signed {
            state: SignedChannelState::Closing { .. },
            ..
        }
    ));
}

fn check_rollover_position(app: &AppHandle, new_expiry: OffsetDateTime) -> bool {
    let position = app.rx.position().unwrap();
    tracing::debug!(
        "expect {:?} to be {:?}",
        position.position_state,
        PositionState::Rollover
    );
    tracing::debug!(
        "expect {} to be {}",
        position.expiry.unix_timestamp(),
        new_expiry.unix_timestamp()
    );

    PositionState::Rollover == position.position_state
        && new_expiry.unix_timestamp() == position.expiry.unix_timestamp()
}
