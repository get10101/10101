use crate::setup;
use crate::setup::dummy_order;
use crate::wait_until;
use native::api;
use native::trade::position::PositionState;
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
    wait_until!(test.app.rx.position_close().is_some());

    // TODO: Assert that the position is closed in the coordinator
}
