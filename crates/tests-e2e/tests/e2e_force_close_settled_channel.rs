#![allow(clippy::unwrap_used)]

use native::api::ChannelState;
use native::api::Direction;
use tests_e2e::app::force_close_dlc_channel;
use tests_e2e::app::get_dlc_channels;
use tests_e2e::app::refresh_wallet_info;
use tests_e2e::app::submit_order;
use tests_e2e::setup;
use tests_e2e::setup::dummy_order;
use tests_e2e::wait_until;

#[tokio::test(flavor = "multi_thread")]
#[ignore = "need to be run with 'just e2e' command"]
#[should_panic]
async fn can_force_close_settled_channel() {
    let setup = setup::TestSetup::new_with_open_position().await;

    let closing_order = {
        let mut order = dummy_order();
        order.direction = Direction::Short;
        order
    };

    submit_order(closing_order.clone());
    wait_until!(setup.app.rx.position_close().is_some());

    let app_balance_before = setup.app.rx.wallet_info().unwrap().balances.on_chain;

    force_close_dlc_channel(&setup.bitcoind).await;

    let channels = get_dlc_channels();
    let channel = channels.first().unwrap();

    wait_until!(matches!(
        dbg!(&channel.channel_state),
        ChannelState::Closed { .. }
    ));

    setup.bitcoind.mine(288).await.unwrap();

    refresh_wallet_info();

    let app_balance_after = setup.app.rx.wallet_info().unwrap().balances.on_chain;

    assert!(app_balance_before < app_balance_after);
}
