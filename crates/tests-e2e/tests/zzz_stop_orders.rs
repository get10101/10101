// XXX: This file starts with `zzz` as the tests are run in alphabetical order.
// This test kills and starts the maker process again, and we do not want to interfere with other
// tests.

use native::api::ContractSymbol;
use orderbook_commons::Prices;
use tests_e2e::process::is_maker_running;
use tests_e2e::process::kill_process;
use tests_e2e::setup::TestSetup;
use tests_e2e::wait_until;

fn has_any_price(prices: &Option<Prices>) -> bool {
    // Hardcoded to BTCUSD
    if let Some(prices) = prices {
        assert!(prices.len() == 1, "only BTCUSD is supported for now");
        let btcusd = prices
            .get(&ContractSymbol::BtcUsd)
            .expect("BTCUSD price to be present");
        return btcusd.bid.is_some() || btcusd.ask.is_some();
    }
    false
}

/// Test that there are no orders when the maker is offline
#[tokio::test]
#[ignore = "need to be run with 'just e2e' command"]
async fn no_orders_when_counterparty_is_offline() {
    let setup = TestSetup::new_after_funding().await;
    wait_until!(has_any_price(&setup.app.rx.prices()));

    tracing::info!("Killing maker process, so that it does not send any orders");
    kill_process("maker").expect("to be able to kill maker process");
    assert!(!is_maker_running(), "maker should be stopped by now");

    // Wait for the orders to expire (30s + up to 60s in the worst case in wait_until!)
    let wait_time = std::time::Duration::from_secs(30);
    tokio::time::sleep(wait_time).await;

    tracing::info!(
        "Waited {:?} secs, all orders should be expired now",
        wait_time.as_secs()
    );

    // FIXME: Adjust the maker's order expiration time to 5 secs, so we don't
    // need to wait as long.
    wait_until!(!has_any_price(&setup.app.rx.prices()));
    assert!(
        !has_any_price(&setup.app.rx.prices()),
        "after the maker is down for over a minute there should be no order"
    );

    tracing::warn!("You need to start the maker process again, otherwise other tests will fail. The easiest way is to run `just e2e` again when testing");
}
