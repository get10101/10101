use crate::app::run_app;
use crate::app::AppHandle;
use crate::bitcoind::Bitcoind;
use crate::coordinator::Coordinator;
use crate::fund::fund_app_with_faucet;
use crate::http::init_reqwest;
use crate::logger::init_tracing;
use crate::wait_until;
use bitcoin::Address;
use bitcoin::Amount;
use native::api;
use native::api::ContractSymbol;
use native::trade::order::api::NewOrder;
use native::trade::order::api::OrderType;
use native::trade::position::PositionState;
use std::str::FromStr;
use tokio::task::spawn_blocking;

pub struct TestSetup {
    pub app: AppHandle,
    pub coordinator: Coordinator,
}

impl TestSetup {
    /// Start test with a running app and a funded wallet.
    pub async fn new_after_funding() -> Self {
        init_tracing();
        let client = init_reqwest();
        let coordinator = Coordinator::new_local(client.clone());
        assert!(coordinator.is_running().await);
        // ensure coordinator has a free UTXO available
        let address = coordinator
            .get_new_address()
            .await
            .expect("To be able to get a new address from coordinator");
        let bitcoind = Bitcoind::new(client.clone());
        bitcoind
            .send_to_address(
                Address::from_str(address.as_str())
                    .expect("To be able to parse address string to address"),
                Amount::ONE_BTC,
            )
            .await
            .expect("To be able to send to address");
        bitcoind.mine(1).await.expect("To be able to mine a block");
        coordinator
            .sync_wallet()
            .await
            .expect("To be able to sync coordinator wallet");

        let app = run_app().await;
        let funded_amount = fund_app_with_faucet(&client, 50_000)
            .await
            .expect("to be able to fund");

        // FIXME: Waiting here on >= as this test run on the CI can't find a route when trying to
        // pay immediately after claiming a received payment.
        // See: https://github.com/get10101/10101/issues/883
        let ln_balance = app
            .rx
            .wallet_info()
            .expect("to have wallet info")
            .balances
            .lightning;
        tracing::info!(%funded_amount, %ln_balance, "Successfully funded app with faucet");
        wait_until!(
            app.rx
                .wallet_info()
                .expect("have wallet_info")
                .balances
                .lightning
                >= funded_amount
        );

        Self { app, coordinator }
    }

    /// Start test with a running app with a funded wallet and an open position.
    pub async fn new_with_open_position() -> Self {
        let setup = Self::new_after_funding().await;
        let rx = &setup.app.rx;

        tracing::info!("Opening a position");
        let order = dummy_order();
        spawn_blocking({
            let order = order.clone();
            move || api::submit_order(order).expect("to submit order")
        })
        .await
        .expect("to spawn_blocking");

        wait_until!(rx.order().is_some());

        wait_until!(rx.position().is_some());
        wait_until!(rx.position().expect("to have position").position_state == PositionState::Open);

        tokio::time::sleep(std::time::Duration::from_secs(10)).await; // wait for coordinator to open position

        setup
    }
}

pub fn dummy_order() -> NewOrder {
    NewOrder {
        leverage: 2.0,
        contract_symbol: ContractSymbol::BtcUsd,
        direction: api::Direction::Long,
        quantity: 1.0,
        order_type: Box::new(OrderType::Market),
    }
}
