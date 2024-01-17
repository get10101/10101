#[tokio::test(flavor = "multi_thread")]
#[ignore = "need to be run with 'just e2e' command"]
async fn app_can_be_restored_from_a_backup() {
    // TODO: this should be re-implemented
    // init_tracing();
    //
    // let test = setup::TestSetup::new_with_open_position().await;
    //
    // let seed_phrase = api::get_seed_phrase();
    //
    // let off_chain = test.app.rx.wallet_info().unwrap().balances.off_chain;
    //
    // // kill the app
    // test.app.stop();
    // tracing::info!("Shutting down app!");
    //
    // let app = run_app(Some(seed_phrase.0)).await;
    //
    // assert_eq!(app.rx.wallet_info().unwrap().balances.off_chain, off_chain);
    //
    // let positions = spawn_blocking(|| api::get_positions().unwrap())
    //     .await
    //     .unwrap();
    // assert_eq!(1, positions.len());
    //
    // // Test if full backup is running without errors
    // spawn_blocking(|| api::full_backup().unwrap())
    //     .await
    //     .unwrap();
    //
    // wait_until!(test.app.rx.channel_status().unwrap() == ChannelStatus::WithPosition);
    //ip

    // let closing_order = {
    //     let mut order = dummy_order();
    //     order.direction = api::Direction::Short;
    //     order
    // };
    //
    // tracing::info!("Closing a position");
    // spawn_blocking(move || api::submit_order(closing_order).unwrap())
    //     .await
    //     .unwrap();
    //
    // wait_until!(test.app.rx.position().unwrap().position_state == PositionState::Closing);
    // wait_until!(test.app.rx.position_close().is_some());
}
