use native::api;
use tests_e2e::setup;
use tests_e2e::wait_until;
use tokio::task::spawn_blocking;

#[tokio::test]
#[ignore = "need to be run with 'just e2e' command"]
async fn can_send_payment_with_open_position() {
    let test = setup::TestSetup::new_with_open_position().await;
    let app = &test.app;

    let ln_balance_before = app.rx.wallet_info().unwrap().balances.lightning;
    let invoice_amount = 10_000;

    tracing::info!("Create an invoice in the coordinator");
    let invoice = test
        .coordinator
        .create_invoice(Some(invoice_amount))
        .await
        .unwrap();

    tracing::info!("Sending payment to coordinator from the app");
    spawn_blocking(move || api::send_payment(invoice.to_string()).unwrap())
        .await
        .unwrap();

    wait_until!(app.rx.wallet_info().unwrap().balances.lightning < ln_balance_before);
    assert!(app.rx.wallet_info().unwrap().balances.lightning <= ln_balance_before - invoice_amount);
}
