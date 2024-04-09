use crate::app::refresh_wallet_info;
use crate::app::run_app;
use crate::app::submit_channel_opening_order;
use crate::app::AppHandle;
use crate::bitcoind::Bitcoind;
use crate::coordinator::Coordinator;
use crate::http::init_reqwest;
use crate::logger::init_tracing;
use crate::wait_until;
use bitcoin::address::NetworkUnchecked;
use bitcoin::Address;
use bitcoin::Amount;
use ln_dlc_node::node::rust_dlc_manager::manager::NB_CONFIRMATIONS;
use native::api;
use native::api::ContractSymbol;
use native::trade::order::api::NewOrder;
use native::trade::order::api::OrderType;
use native::trade::position::PositionState;

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
            Some(0),
            "App should start with empty off-chain wallet"
        );

        Self {
            app,
            coordinator,
            bitcoind,
        }
    }

    /// Funds the coordinator with [`amount`/`n_utxos`] utxos
    ///
    /// E.g. if amount = 3 BTC, and n_utxos = 3, it would create 3 UTXOs a 1 BTC
    pub async fn fund_coordinator(&self, amount: Amount, n_utxos: u64) {
        // Ensure that the coordinator has a free UTXO available.
        let address = self
            .coordinator
            .get_new_address()
            .await
            .unwrap()
            .assume_checked();

        let sats_per_fund = amount.to_sat() / n_utxos;
        for _ in 0..n_utxos {
            self.bitcoind
                .send_to_address(&address, Amount::from_sat(sats_per_fund))
                .await
                .unwrap();
        }

        self.bitcoind.mine(1).await.unwrap();

        self.sync_coordinator().await;

        // TODO: Get coordinator balance to verify this claim.
        tracing::info!("Successfully funded coordinator");
    }

    pub async fn fund_app(&self, fund_amount: Amount) {
        let address = api::get_new_address().unwrap();
        let address: Address<NetworkUnchecked> = address.parse().unwrap();

        self.bitcoind
            .send_to_address(&address.assume_checked(), fund_amount)
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

        setup.fund_coordinator(Amount::ONE_BTC, 2).await;

        setup.fund_app(Amount::ONE_BTC).await;

        setup
    }

    /// Start test with a running app with a funded wallet and an open position.
    pub async fn new_with_open_position() -> Self {
        let order = dummy_order();

        Self::new_with_open_position_custom(order, 0, 0).await
    }

    /// Start test with a running app with a funded wallet and an open position based on a custom
    /// [`NewOrder`].
    pub async fn new_with_open_position_custom(
        order: NewOrder,
        coordinator_collateral_reserve: u64,
        trader_collateral_reserve: u64,
    ) -> Self {
        let setup = Self::new_after_funding().await;
        let rx = &setup.app.rx;

        tracing::info!(
            ?order,
            %coordinator_collateral_reserve,
            %trader_collateral_reserve,
            "Opening a position"
        );

        submit_channel_opening_order(
            order.clone(),
            coordinator_collateral_reserve,
            trader_collateral_reserve,
        );

        wait_until!(rx.order().is_some());

        wait_until!(rx.position().is_some());
        wait_until!(rx.position().unwrap().position_state == PositionState::Open);

        // Wait for coordinator to open position.
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;

        setup.bitcoind.mine(NB_CONFIRMATIONS as u16).await.unwrap();

        tokio::time::sleep(std::time::Duration::from_secs(10)).await;

        // Includes on-chain sync and DLC channels sync.
        refresh_wallet_info();

        setup.sync_coordinator().await;

        setup
    }

    async fn sync_coordinator(&self) {
        if let Err(e) = self.coordinator.sync_node().await {
            tracing::error!("Got error from coordinator sync: {e:#}");
        };
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
