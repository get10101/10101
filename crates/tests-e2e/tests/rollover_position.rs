use native::api;
use native::trade::position;
use position::PositionState;
use tests_e2e::app::AppHandle;
use tests_e2e::setup;
use tests_e2e::wait_until;
use time::Duration;
use time::OffsetDateTime;

#[tokio::test]
#[ignore]
async fn can_rollover_position() {
    let test = setup::TestSetup::new_with_open_position().await;
    let coordinator = &test.coordinator;
    let dlc_channels = coordinator.get_dlc_channels().await.unwrap();
    let app_pubkey = api::get_node_id().unwrap().0;

    tracing::info!("{:?}", dlc_channels);

    let dlc_channel = dlc_channels
        .into_iter()
        .find(|chan| chan.counter_party == app_pubkey)
        .unwrap();

    let position = test.app.rx.position().expect("position to exist");
    let tomorrow = position.expiry.date() + Duration::days(7);
    let new_expiry = tomorrow.midnight().assume_utc();

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
}

fn check_rollover_position(app: &AppHandle, new_expiry: OffsetDateTime) -> bool {
    let position = app.rx.position().expect("position to exist");
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
