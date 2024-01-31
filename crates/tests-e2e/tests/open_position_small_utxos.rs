use bitcoin::Amount;
use native::api;
use native::api::calculate_margin;
use native::api::ContractSymbol;
use native::trade::order::api::NewOrder;
use native::trade::order::api::OrderType;
use native::trade::position::PositionState;
use rust_decimal::prelude::ToPrimitive;
use std::str::FromStr;
use tests_e2e::app::refresh_wallet_info;
use tests_e2e::setup::TestSetup;
use tests_e2e::wait_until;
use tokio::task::spawn_blocking;

#[tokio::test(flavor = "multi_thread")]
#[ignore = "need to be run with 'just e2e' command"]
async fn can_open_position_with_multiple_small_utxos() {
    // Arrange

    let setup = TestSetup::new().await;

    setup.fund_coordinator(Amount::ONE_BTC).await;

    let app = &setup.app;

    // Fund app with multiple small UTXOs that can cover the required margin.

    let order = NewOrder {
        leverage: 2.0,
        contract_symbol: ContractSymbol::BtcUsd,
        direction: api::Direction::Long,
        quantity: 100.0,
        order_type: Box::new(OrderType::Market),
        stable: false,
    };

    // We take the ask price because the app is going long.
    let ask_price = app
        .rx
        .prices()
        .unwrap()
        .get(&ContractSymbol::BtcUsd)
        .unwrap()
        .ask
        .unwrap()
        .to_f32()
        .unwrap();

    let margin_app = calculate_margin(ask_price, order.quantity, order.leverage).0;

    // We want to use small UTXOs.
    let utxo_size = 1_000;

    let n_utxos = margin_app / utxo_size;

    // Double the number of UTXOs to cover costs beyond the margin i.e. fees.
    let n_utxos = 2 * n_utxos;

    let address_fn = || bitcoin::Address::from_str(&api::get_new_address().unwrap()).unwrap();

    setup
        .bitcoind
        .send_multiple_utxos_to_address(address_fn, Amount::from_sat(utxo_size), n_utxos)
        .await
        .unwrap();

    let fund_amount = n_utxos * utxo_size;

    setup.bitcoind.mine(1).await.unwrap();

    wait_until!({
        refresh_wallet_info();
        app.rx.wallet_info().unwrap().balances.on_chain >= fund_amount
    });

    // Act

    spawn_blocking({
        let order = order.clone();
        move || api::submit_order(order).unwrap()
    })
    .await
    .unwrap();

    // Assert

    wait_until!(matches!(
        app.rx.position(),
        Some(native::trade::position::Position {
            position_state: PositionState::Open,
            ..
        })
    ));
}
