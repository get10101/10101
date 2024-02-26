use crate::bitcoin_conversion::to_secp_pk_29;
use crate::node::InMemoryStore;
use crate::node::Node;
use crate::node::RunningNode;
use crate::on_chain_wallet;
use crate::storage::TenTenOneInMemoryStorage;
use crate::tests::bitcoind::mine;
use crate::tests::dummy_contract_input;
use crate::tests::init_tracing;
use crate::tests::new_reference_id;
use crate::tests::wait_until;
use bitcoin::Amount;
use dlc_manager::channel::signed_channel::SignedChannel;
use dlc_manager::channel::signed_channel::SignedChannelStateType;
use dlc_manager::contract::Contract;
use dlc_manager::Storage;
use std::sync::Arc;
use std::time::Duration;

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn can_open_and_settle_offchain() {
    init_tracing();

    let (
        (app, _running_app),
        (coordinator, _running_coordinator),
        app_signed_channel,
        coordinator_signed_channel,
    ) = set_up_channel_with_position().await;

    let oracle_pk = *coordinator.oracle_pk().first().unwrap();
    let contract_input = dummy_contract_input(15_000, 5_000, oracle_pk, None);

    coordinator
        .propose_dlc_channel_update(
            &coordinator_signed_channel.channel_id,
            contract_input,
            new_reference_id(),
        )
        .await
        .unwrap();

    wait_until(Duration::from_secs(10), || async {
        app.process_incoming_messages()?;

        let dlc_channels = app
            .dlc_manager
            .get_store()
            .get_signed_channels(Some(SignedChannelStateType::RenewOffered))?;

        Ok(dlc_channels
            .iter()
            .find(|dlc_channel| dlc_channel.counter_party == to_secp_pk_29(coordinator.info.pubkey))
            .cloned())
    })
    .await
    .unwrap();

    app.accept_dlc_channel_update(&app_signed_channel.channel_id)
        .unwrap();

    wait_until(Duration::from_secs(10), || async {
        coordinator.process_incoming_messages()?;

        let dlc_channels = coordinator
            .dlc_manager
            .get_store()
            .get_signed_channels(Some(SignedChannelStateType::RenewConfirmed))?;

        Ok(dlc_channels
            .iter()
            .find(|dlc_channel| dlc_channel.counter_party == to_secp_pk_29(app.info.pubkey))
            .cloned())
    })
    .await
    .unwrap();

    wait_until(Duration::from_secs(10), || async {
        app.process_incoming_messages()?;

        let dlc_channels = app
            .dlc_manager
            .get_store()
            .get_signed_channels(Some(SignedChannelStateType::RenewFinalized))?;

        Ok(dlc_channels
            .iter()
            .find(|dlc_channel| dlc_channel.counter_party == to_secp_pk_29(coordinator.info.pubkey))
            .cloned())
    })
    .await
    .unwrap();

    wait_until(Duration::from_secs(10), || async {
        coordinator.process_incoming_messages()?;

        let dlc_channels = coordinator
            .dlc_manager
            .get_store()
            .get_signed_channels(Some(SignedChannelStateType::Established))?;

        Ok(dlc_channels
            .iter()
            .find(|dlc_channel| dlc_channel.counter_party == to_secp_pk_29(app.info.pubkey))
            .cloned())
    })
    .await
    .unwrap();

    wait_until(Duration::from_secs(10), || async {
        app.process_incoming_messages()?;

        let dlc_channels = app
            .dlc_manager
            .get_store()
            .get_signed_channels(Some(SignedChannelStateType::Established))?;

        Ok(dlc_channels
            .iter()
            .find(|dlc_channel| dlc_channel.counter_party == to_secp_pk_29(coordinator.info.pubkey))
            .cloned())
    })
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn can_open_and_collaboratively_close_channel() {
    init_tracing();

    let (
        (app, _running_app),
        (coordinator, _running_coordinator),
        app_signed_channel,
        coordinator_signed_channel,
    ) = set_up_channel_with_position().await;

    let app_on_chain_balance_before_close = app.get_on_chain_balance();
    let coordinator_on_chain_balance_before_close = coordinator.get_on_chain_balance();

    tracing::debug!("Proposing to close dlc channel collaboratively");

    coordinator
        .close_dlc_channel(app_signed_channel.channel_id, false)
        .await
        .unwrap();

    wait_until(Duration::from_secs(10), || async {
        app.process_incoming_messages()?;

        let dlc_channels = app
            .dlc_manager
            .get_store()
            .get_signed_channels(Some(SignedChannelStateType::CollaborativeCloseOffered))?;

        Ok(dlc_channels
            .iter()
            .find(|dlc_channel| dlc_channel.counter_party == to_secp_pk_29(coordinator.info.pubkey))
            .cloned())
    })
    .await
    .unwrap();

    tracing::debug!("Accepting collaborative close offer");

    app.accept_dlc_channel_collaborative_close(&coordinator_signed_channel.channel_id)
        .unwrap();

    wait_until(Duration::from_secs(10), || async {
        mine(1).await.unwrap();
        coordinator.sync_wallets().await?;

        let coordinator_on_chain_balances_after_close = coordinator.get_on_chain_balance();

        let coordinator_balance_changed = coordinator_on_chain_balances_after_close.confirmed
            > coordinator_on_chain_balance_before_close.confirmed;

        if coordinator_balance_changed {
            tracing::debug!(
                old_balance = coordinator_on_chain_balance_before_close.confirmed,
                new_balance = coordinator_on_chain_balances_after_close.confirmed,
                "Balance updated"
            )
        }

        Ok(coordinator_balance_changed.then_some(true))
    })
    .await
    .unwrap();

    wait_until(Duration::from_secs(10), || async {
        mine(1).await.unwrap();
        app.sync_wallets().await?;

        let app_on_chain_balances_after_close = app.get_on_chain_balance();

        let app_balance_changed = app_on_chain_balances_after_close.confirmed
            > app_on_chain_balance_before_close.confirmed;
        if app_balance_changed {
            tracing::debug!(
                old_balance = app_on_chain_balance_before_close.confirmed,
                new_balance = app_on_chain_balances_after_close.confirmed,
                "Balance updated"
            )
        }

        Ok(app_balance_changed.then_some(()))
    })
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn can_open_and_force_close_channel() {
    init_tracing();

    let ((app, _running_app), (coordinator, _running_coordinator), _, coordinator_signed_channel) =
        set_up_channel_with_position().await;

    tracing::debug!("Force-closing DLC channel");

    wait_until(Duration::from_secs(10), || async {
        mine(1).await.unwrap();

        let dlc_channels = coordinator
            .dlc_manager
            .get_store()
            .get_signed_channels(None)?;
        Ok(dlc_channels
            .iter()
            .find(|dlc_channel| dlc_channel.counter_party == to_secp_pk_29(app.info.pubkey))
            .cloned())
    })
    .await
    .unwrap();

    coordinator
        .close_dlc_channel(coordinator_signed_channel.channel_id, true)
        .await
        .unwrap();

    wait_until(Duration::from_secs(10), || async {
        mine(1).await.unwrap();

        let dlc_channels = coordinator
            .dlc_manager
            .get_store()
            .get_signed_channels(None)?;
        Ok(dlc_channels.is_empty().then_some(()))
    })
    .await
    .unwrap();

    // TODO: we could also test that the DLCs are being spent, but for that we would need a TARDIS
    // or similar
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn can_open_channel_with_min_inputs() {
    init_tracing();

    let app_dlc_collateral = Amount::from_sat(10_000);
    let coordinator_dlc_collateral = Amount::from_sat(10_000);

    // We must fix the fee rate so that we can predict how many sats `rust-dlc` will allocate
    // for transaction fees.
    let fee_rate_sats_per_vbyte = 2;
    let expected_fund_tx_fee = 252 * fee_rate_sats_per_vbyte;

    // This also depends on the fee rate, but the formula is a bit more involved.
    let fee_reserve = 880;

    // Fee costs are evenly split.
    let fee_cost_per_party = (expected_fund_tx_fee + fee_reserve) / 2;
    let fee_cost_per_party = Amount::from_sat(fee_cost_per_party);

    let (app, _running_app) = start_and_fund_app(app_dlc_collateral + fee_cost_per_party, 1).await;
    let (coordinator, _running_coordinator) =
        start_and_fund_coordinator(coordinator_dlc_collateral + fee_cost_per_party, 1).await;

    let (app_signed_channel, _) = open_channel_and_position(
        app.clone(),
        coordinator.clone(),
        app_dlc_collateral,
        coordinator_dlc_collateral,
        Some(fee_rate_sats_per_vbyte),
    )
    .await;

    // No change output means that the inputs were spent in full by the fund output.
    assert!(app_signed_channel.fund_tx.output.len() == 1);
}

async fn start_and_fund_app(
    amount: Amount,
    n_utxos: u64,
) -> (
    Arc<Node<on_chain_wallet::InMemoryStorage, TenTenOneInMemoryStorage, InMemoryStore>>,
    RunningNode,
) {
    let (node, running_node) = Node::start_test_app("app").unwrap();

    node.fund(amount, n_utxos).await.unwrap();

    (node, running_node)
}

async fn start_and_fund_coordinator(
    amount: Amount,
    n_utxos: u64,
) -> (
    Arc<Node<on_chain_wallet::InMemoryStorage, TenTenOneInMemoryStorage, InMemoryStore>>,
    RunningNode,
) {
    let (node, running_node) = Node::start_test_coordinator("coordinator").unwrap();

    node.fund(amount, n_utxos).await.unwrap();

    (node, running_node)
}

async fn set_up_channel_with_position() -> (
    (
        Arc<Node<on_chain_wallet::InMemoryStorage, TenTenOneInMemoryStorage, InMemoryStore>>,
        RunningNode,
    ),
    (
        Arc<Node<on_chain_wallet::InMemoryStorage, TenTenOneInMemoryStorage, InMemoryStore>>,
        RunningNode,
    ),
    SignedChannel,
    SignedChannel,
) {
    let app_dlc_collateral = Amount::from_sat(10_000);
    let coordinator_dlc_collateral = Amount::from_sat(10_000);

    let (app, running_app) = start_and_fund_app(Amount::from_sat(10_000_000), 10).await;
    let (coordinator, running_coordinator) =
        start_and_fund_coordinator(Amount::from_sat(10_000_000), 10).await;

    let (app_signed_channel, coordinator_signed_channel) = open_channel_and_position(
        app.clone(),
        coordinator.clone(),
        app_dlc_collateral,
        coordinator_dlc_collateral,
        None,
    )
    .await;

    (
        (app, running_app),
        (coordinator, running_coordinator),
        app_signed_channel,
        coordinator_signed_channel,
    )
}

async fn open_channel_and_position(
    app: Arc<Node<on_chain_wallet::InMemoryStorage, TenTenOneInMemoryStorage, InMemoryStore>>,
    coordinator: Arc<
        Node<on_chain_wallet::InMemoryStorage, TenTenOneInMemoryStorage, InMemoryStore>,
    >,
    app_dlc_collateral: Amount,
    coordinator_dlc_collateral: Amount,
    fee_rate_sats_per_vbyte: Option<u64>,
) -> (SignedChannel, SignedChannel) {
    app.connect(coordinator.info).await.unwrap();

    let app_balance_before_sat = app.get_on_chain_balance().confirmed;
    let coordinator_balance_before_sat = coordinator.get_on_chain_balance().confirmed;

    let oracle_pk = *coordinator.oracle_pk().first().unwrap();
    let contract_input = dummy_contract_input(
        app_dlc_collateral.to_sat(),
        coordinator_dlc_collateral.to_sat(),
        oracle_pk,
        fee_rate_sats_per_vbyte,
    );

    coordinator
        .propose_dlc_channel(contract_input, app.info.pubkey, new_reference_id())
        .await
        .unwrap();

    let offered_channel = wait_until(Duration::from_secs(30), || async {
        app.process_incoming_messages()?;

        let dlc_channels = app.dlc_manager.get_store().get_offered_channels()?;

        Ok(dlc_channels
            .iter()
            .find(|dlc_channel| dlc_channel.counter_party == to_secp_pk_29(coordinator.info.pubkey))
            .cloned())
    })
    .await
    .unwrap();

    app.accept_dlc_channel_offer(&offered_channel.temporary_channel_id)
        .unwrap();

    let coordinator_signed_channel = wait_until(Duration::from_secs(30), || async {
        coordinator.process_incoming_messages()?;

        let dlc_channels = coordinator
            .dlc_manager
            .get_store()
            .get_signed_channels(None)?;

        Ok(dlc_channels
            .iter()
            .find(|dlc_channel| dlc_channel.counter_party == to_secp_pk_29(app.info.pubkey))
            .cloned())
    })
    .await
    .unwrap();

    let app_signed_channel = wait_until(Duration::from_secs(30), || async {
        app.process_incoming_messages()?;

        let dlc_channels = app.dlc_manager.get_store().get_signed_channels(None)?;

        Ok(dlc_channels
            .iter()
            .find(|dlc_channel| dlc_channel.counter_party == to_secp_pk_29(coordinator.info.pubkey))
            .cloned())
    })
    .await
    .unwrap();

    // FIXME(holzeis): `Chopsticks automatically mined an additional block when calling its API. now
    // that we have removed chopsticks this acutally translates to mining 2 blocks instead of just
    // 1. related to https://github.com/get10101/10101/issues/1990`
    mine(dlc_manager::manager::NB_CONFIRMATIONS as u16 + 1)
        .await
        .unwrap();

    wait_until(Duration::from_secs(30), || async {
        app.sync_wallets().await.unwrap();

        let app_balance_after_open_sat = app.get_on_chain_balance().confirmed;

        // We don't aim to account for transaction fees exactly.
        Ok(
            (app_balance_after_open_sat <= app_balance_before_sat - app_dlc_collateral.to_sat())
                .then_some(()),
        )
    })
    .await
    .unwrap();

    wait_until(Duration::from_secs(30), || async {
        coordinator.sync_wallets().await.unwrap();

        let coordinator_balance_after_open_sat = coordinator.get_on_chain_balance().confirmed;

        // We don't aim to account for transaction fees exactly.
        Ok((coordinator_balance_after_open_sat
            <= coordinator_balance_before_sat - coordinator_dlc_collateral.to_sat())
        .then_some(()))
    })
    .await
    .unwrap();

    wait_until(Duration::from_secs(30), || async {
        if let Err(e) = app.dlc_manager.periodic_chain_monitor() {
            tracing::error!("Failed to run DLC manager periodic chain monitor task: {e:#}");
        };
        if let Err(e) = app.dlc_manager.periodic_check() {
            tracing::error!("Failed to run DLC manager periodic check: {e:#}");
        };

        let contract = app
            .dlc_manager
            .get_store()
            .get_contract(&app_signed_channel.get_contract_id().unwrap())
            .unwrap();

        Ok(matches!(contract, Some(Contract::Confirmed(_))).then_some(()))
    })
    .await
    .unwrap();

    wait_until(Duration::from_secs(30), || async {
        coordinator.dlc_manager.periodic_chain_monitor().unwrap();
        coordinator.dlc_manager.periodic_check().unwrap();

        let contract = coordinator
            .dlc_manager
            .get_store()
            .get_contract(&coordinator_signed_channel.get_contract_id().unwrap())
            .unwrap();

        Ok(matches!(contract, Some(Contract::Confirmed(_))).then_some(()))
    })
    .await
    .unwrap();

    tracing::info!("DLC channel is on-chain");

    coordinator
        .propose_dlc_channel_collaborative_settlement(
            &coordinator_signed_channel.channel_id,
            coordinator_dlc_collateral.to_sat() / 2,
            new_reference_id(),
        )
        .await
        .unwrap();

    tracing::debug!("Waiting for settle offer...");
    let app_signed_channel = wait_until(Duration::from_secs(30), || async {
        app.process_incoming_messages()?;

        let dlc_channels = app
            .dlc_manager
            .get_store()
            .get_signed_channels(Some(SignedChannelStateType::SettledReceived))?;

        Ok(dlc_channels
            .iter()
            .find(|dlc_channel| dlc_channel.counter_party == to_secp_pk_29(coordinator.info.pubkey))
            .cloned())
    })
    .await
    .unwrap();

    tracing::debug!("Accepting settle offer and waiting for being settled...");
    app.accept_dlc_channel_collaborative_settlement(&app_signed_channel.channel_id)
        .unwrap();

    wait_until(Duration::from_secs(10), || async {
        app.process_incoming_messages()?;

        let dlc_channels = app
            .dlc_manager
            .get_store()
            .get_signed_channels(Some(SignedChannelStateType::SettledAccepted))?;

        Ok(dlc_channels
            .iter()
            .find(|dlc_channel| dlc_channel.counter_party == to_secp_pk_29(coordinator.info.pubkey))
            .cloned())
    })
    .await
    .unwrap();

    wait_until(Duration::from_secs(10), || async {
        coordinator.process_incoming_messages()?;

        let dlc_channels = coordinator
            .dlc_manager
            .get_store()
            .get_signed_channels(Some(SignedChannelStateType::SettledConfirmed))?;

        Ok(dlc_channels
            .iter()
            .find(|dlc_channel| dlc_channel.counter_party == to_secp_pk_29(app.info.pubkey))
            .cloned())
    })
    .await
    .unwrap();

    wait_until(Duration::from_secs(10), || async {
        app.process_incoming_messages()?;

        let dlc_channels = app
            .dlc_manager
            .get_store()
            .get_signed_channels(Some(SignedChannelStateType::Settled))?;

        Ok(dlc_channels
            .iter()
            .find(|dlc_channel| dlc_channel.counter_party == to_secp_pk_29(coordinator.info.pubkey))
            .cloned())
    })
    .await
    .unwrap();

    wait_until(Duration::from_secs(10), || async {
        coordinator.process_incoming_messages()?;

        let dlc_channels = coordinator
            .dlc_manager
            .get_store()
            .get_signed_channels(Some(SignedChannelStateType::Settled))?;

        Ok(dlc_channels
            .iter()
            .find(|dlc_channel| dlc_channel.counter_party == to_secp_pk_29(app.info.pubkey))
            .cloned())
    })
    .await
    .unwrap();

    (app_signed_channel, coordinator_signed_channel)
}
