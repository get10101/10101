use native::api;
use native::trade::position::PositionState;
use tests_e2e::fund::pay_with_faucet;
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

    tracing::info!("Position closed");

    // Ensure we sync the wallet info after funding
    spawn_blocking(move || api::refresh_wallet_info().expect("to succeed"))
        .await
        .unwrap();

    let balance_at_closing = test.app.rx.wallet_info().unwrap().balances.on_chain;

    let invoice =
        spawn_blocking(move || api::create_invoice_with_amount(50_000).expect("to succeed"))
            .await
            .unwrap();
    api::decode_invoice(invoice.clone()).expect("to decode invoice we created");

    let client = reqwest::Client::new();
    pay_with_faucet(&client, invoice).await.unwrap();

    wait_until!(test.app.rx.wallet_info().unwrap().balances.on_chain > balance_at_closing);
}
