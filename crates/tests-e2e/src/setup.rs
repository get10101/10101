use crate::app::refresh_wallet_info;
use crate::app::run_app;
use crate::app::sync_dlc_channels;
use crate::app::AppHandle;
use crate::bitcoind::Bitcoind;
use crate::coordinator::Coordinator;
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
    pub async fn new() -> Self {
        init_tracing();

        let client = init_reqwest();
        let bitcoind = Bitcoind::new_local(client.clone());

        // Coordinator setup

        let coordinator = Coordinator::new_local(client.clone());

        assert!(coordinator.is_running().await);

        // App setup

        let app = run_app(None).await;

        assert_eq!(
            app.rx.wallet_info().unwrap().balances.on_chain,
            0,
            "App should start with empty on-chain wallet"
        );

        assert_eq!(
            app.rx.wallet_info().unwrap().balances.off_chain,
            0,
            "App should start with empty off-chain wallet"
        );

        Self {
            app,
            coordinator,
            bitcoind,
        }
    }

    pub async fn fund_coordinator(&self, amount: Amount) {
        // Ensure that the coordinator has a free UTXO available.
        let address = self.coordinator.get_new_address().await.unwrap();

        self.bitcoind
            .send_to_address(&address, amount)
            .await
            .unwrap();

        self.bitcoind.mine(1).await.unwrap();
        self.coordinator.sync_node().await.unwrap();

        // TODO: Get coordinator balance to verify this claim.
        tracing::info!("Successfully funded coordinator");
    }

    pub async fn fund_app(&self, fund_amount: Amount) {
        let address = api::get_unused_address();
        let address = &address.0.parse().unwrap();

        self.bitcoind
            .send_to_address(address, fund_amount)
            .await
            .unwrap();

        self.bitcoind.mine(1).await.unwrap();

        wait_until!({
            refresh_wallet_info();
            self.app.rx.wallet_info().unwrap().balances.on_chain >= fund_amount.to_sat()
        });

        let on_chain_balance = self.app.rx.wallet_info().unwrap().balances.on_chain;

        tracing::info!(%fund_amount, %on_chain_balance, "Successfully funded app");
    }

    /// Start test with a running app and a funded wallet.
    pub async fn new_after_funding() -> Self {
        let setup = Self::new().await;

        setup.fund_coordinator(Amount::ONE_BTC).await;

        setup.fund_app(Amount::ONE_BTC).await;

        setup
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

        setup.bitcoind.mine(6).await.unwrap();

        tokio::time::sleep(std::time::Duration::from_secs(10)).await;

        sync_dlc_channels();
        refresh_wallet_info();

        setup.coordinator.sync_node().await.unwrap();

        setup
    }
}

pub fn dummy_order() -> NewOrder {
    NewOrder {
        leverage: 2.0,
        contract_symbol: ContractSymbol::BtcUsd,
        direction: api::Direction::Long,
        quantity: 1000.0,
        order_type: Box::new(OrderType::Market),
        stable: false,
    }
}
