use native::api;
use native::trade::position::PositionState;
use tests_e2e::app::run_app;
use tests_e2e::logger::init_tracing;
use tests_e2e::setup;
use tests_e2e::setup::dummy_order;
use tests_e2e::wait_until;
use tokio::task::spawn_blocking;

#[tokio::test(flavor = "multi_thread")]
#[ignore = "need to be run with 'just e2e' command"]
async fn app_can_be_restored_from_a_backup() {
    init_tracing();

    let test = setup::TestSetup::new_with_open_position().await;

    let seed_phrase = api::get_seed_phrase();

    let off_chain = test.app.rx.wallet_info().unwrap().balances.off_chain;

    // kill the app
    test.app.stop();
    tracing::info!("Shutting down app!");

    let app = run_app(Some(seed_phrase.0)).await;

    assert_eq!(app.rx.wallet_info().unwrap().balances.off_chain, off_chain);

    let positions = spawn_blocking(|| api::get_positions().unwrap())
        .await
        .unwrap();
    assert_eq!(1, positions.len());

    // Test if full backup is running without errors
    spawn_blocking(|| api::full_backup().unwrap())
        .await
        .unwrap();

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
}
