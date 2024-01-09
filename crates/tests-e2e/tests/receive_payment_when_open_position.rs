use native::api;
use tests_e2e::setup;
use tests_e2e::wait_until;
use tokio::task::spawn_blocking;

#[tokio::test]
#[ignore = "need to be run with 'just e2e' command"]
async fn can_receive_payment_with_open_position() {
    let test = setup::TestSetup::new_with_open_position().await;
    let app = &test.app;

    let ln_balance_before = app.rx.wallet_info().unwrap().balances.off_chain;
    let invoice_amount = 10_000;

    tracing::info!("Creating an invoice");
    let invoice = spawn_blocking(move || api::create_onboarding_invoice(1, invoice_amount, 10_000))
        .await
        .unwrap()
        .unwrap();

    tracing::info!("Coordinator pays the invoice of {invoice_amount} sats created in the app");
    test.coordinator.pay_invoice(&invoice).await.unwrap();

    wait_until!(app.rx.wallet_info().unwrap().balances.off_chain > ln_balance_before);
    let ln_balance = app.rx.wallet_info().unwrap().balances.off_chain;
    tracing::info!(%ln_balance, %ln_balance_before, %invoice_amount, "Lightning balance increased");

    assert_eq!(ln_balance, ln_balance_before + invoice_amount);
}
