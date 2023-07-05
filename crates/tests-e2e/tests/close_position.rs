use native::api::{self};
use native::trade::position::PositionState;
use tests_e2e::setup;
use tests_e2e::setup::dummy_order;
use tests_e2e::wait_until;
use tokio::task::spawn_blocking;

#[tokio::test]
#[ignore = "need to be run with 'just e2e' command"]
async fn can_collab_close_position() {
    let test = setup::TestSetup::new_with_open_position().await;

    let closing_order = {
        let mut order = dummy_order();
        order.direction = api::Direction::Short;
        order
    };

    tracing::info!("Closing a position");
    spawn_blocking(move || api::submit_order(closing_order).unwrap())
        .await
        .unwrap();

    wait_until!(test.app.rx.position().unwrap().position_state == PositionState::Closing);

    // TODO: Assert that the position is closed in the app and the coordinator
}
