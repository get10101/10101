use crate::app::run_app;
use crate::app::AppHandle;
use crate::bitcoind::Bitcoind;
use crate::coordinator::Coordinator;
use crate::fund::fund_app_with_faucet;
use crate::http::init_reqwest;
use crate::logger::init_tracing;
use crate::wait_until;
use bitcoin::Amount;
use native::api;
use native::api::ContractSymbol;
use native::trade::order::api::NewOrder;
use native::trade::order::api::OrderType;
use native::trade::position::PositionState;
use tokio::task::spawn_blocking;

pub struct TestSetup {
    pub app: AppHandle,
    pub coordinator: Coordinator,
    pub bitcoind: Bitcoind,
}

impl TestSetup {
    /// Start test with a running app and a funded wallet.
    pub async fn new_after_funding() -> Self {
        init_tracing();
        let client = init_reqwest();
        let coordinator = Coordinator::new_local(client.clone());
        assert!(coordinator.is_running().await);
        // ensure coordinator has a free UTXO available
        let address = coordinator.get_new_address().await.unwrap();
        let bitcoind = Bitcoind::new_local(client.clone());
        bitcoind
            .send_to_address(&address, Amount::ONE_BTC)
            .await
            .unwrap();
        bitcoind.mine(1).await.unwrap();
        coordinator.sync_wallet().await.unwrap();

        let app = run_app(None).await;

        assert_eq!(
            app.rx.wallet_info().unwrap().balances.lightning,
            0,
            "App should start with empty wallet"
        );

        let fund_amount = 50_000;
        fund_app_with_faucet(&app, &client, fund_amount)
            .await
            .unwrap();

        let ln_balance = app.rx.wallet_info().unwrap().balances.lightning;
        tracing::info!(%fund_amount, %ln_balance, "Successfully funded app with faucet");

        assert!(ln_balance > 0, "App wallet should be funded");

        Self {
            app,
            coordinator,
            bitcoind,
        }
    }

    /// Start test with a running app with a funded wallet and an open position.
    pub async fn new_with_open_position() -> Self {
        let setup = Self::new_after_funding().await;
        let rx = &setup.app.rx;

        tracing::info!("Opening a position");
        let order = dummy_order();
        spawn_blocking({
            let order = order.clone();
            move || api::submit_order(order).unwrap()
        })
        .await
        .unwrap();

        wait_until!(rx.order().is_some());

        wait_until!(rx.position().is_some());
        wait_until!(rx.position().unwrap().position_state == PositionState::Open);

        // Wait for coordinator to open position.
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;

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
        stable: false,
    }
}
