use crate::node::InMemoryStore;
use crate::node::Node;
use crate::storage::TenTenOneInMemoryStorage;
use crate::tests::bitcoind::mine;
use crate::tests::dummy_contract_input;
use crate::tests::init_tracing;
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

    // Arrange

    let (app, coordinator, coordinator_signed_channel, app_signed_channel) =
        setup_channel_with_position().await;

    // Act

    let oracle_pk = *coordinator.oracle_pk().first().unwrap();
    let contract_input = dummy_contract_input(15_000, 5_000, oracle_pk);

    coordinator
        .propose_dlc_channel_update(&coordinator_signed_channel.channel_id, contract_input)
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
            .find(|dlc_channel| dlc_channel.counter_party == coordinator.info.pubkey)
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
            .find(|dlc_channel| dlc_channel.counter_party == app.info.pubkey)
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
            .find(|dlc_channel| dlc_channel.counter_party == coordinator.info.pubkey)
            .cloned())
    })
    .await
    .unwrap();

    // Assert

    wait_until(Duration::from_secs(10), || async {
        coordinator.process_incoming_messages()?;

        let dlc_channels = coordinator
            .dlc_manager
            .get_store()
            .get_signed_channels(Some(SignedChannelStateType::Established))?;

        Ok(dlc_channels
            .iter()
            .find(|dlc_channel| dlc_channel.counter_party == app.info.pubkey)
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
            .find(|dlc_channel| dlc_channel.counter_party == coordinator.info.pubkey)
            .cloned())
    })
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn can_open_and_collaboratively_close_channel() {
    init_tracing();

    // Arrange
    let (app, coordinator, coordinator_signed_channel, app_signed_channel) =
        setup_channel_with_position().await;

    let app_on_chain_balance_before_close = app.get_on_chain_balance().unwrap();
    let coordinator_on_chain_balance_before_close = coordinator.get_on_chain_balance().unwrap();

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
            .find(|dlc_channel| dlc_channel.counter_party == coordinator.info.pubkey)
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

        let coordinator_on_chain_balances_after_close = coordinator.get_on_chain_balance()?;

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

        let app_on_chain_balances_after_close = app.get_on_chain_balance()?;

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

    // Arrange
    let (app, coordinator, coordinator_signed_channel, _) = setup_channel_with_position().await;

    tracing::debug!("Force closing dlc channel");

    wait_until(Duration::from_secs(10), || async {
        mine(1).await.unwrap();

        let dlc_channels = coordinator
            .dlc_manager
            .get_store()
            .get_signed_channels(None)?;
        Ok(dlc_channels
            .iter()
            .find(|dlc_channel| dlc_channel.counter_party == app.info.pubkey)
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

async fn setup_channel_with_position() -> (
    Arc<Node<TenTenOneInMemoryStorage, InMemoryStore>>,
    Arc<Node<TenTenOneInMemoryStorage, InMemoryStore>>,
    SignedChannel,
    SignedChannel,
) {
    let app_dlc_collateral = 10_000;
    let coordinator_dlc_collateral = 10_000;

    let (app, _running_app) = Node::start_test_app("app").unwrap();
    let (coordinator, _running_coord) = Node::start_test_coordinator("coordinator").unwrap();

    app.connect(coordinator.info).await.unwrap();

    // Choosing large fund amounts compared to the DLC collateral to ensure that we have one input
    // per party. In the end, it doesn't seem to matter though.

    app.fund(Amount::from_sat(10_000_000)).await.unwrap();

    coordinator
        .fund(Amount::from_sat(10_000_000))
        .await
        .unwrap();

    let app_balance_before = app.get_on_chain_balance().unwrap().confirmed;
    let coordinator_balance_before = coordinator.get_on_chain_balance().unwrap().confirmed;

    // Act

    let oracle_pk = *coordinator.oracle_pk().first().unwrap();
    let contract_input =
        dummy_contract_input(app_dlc_collateral, coordinator_dlc_collateral, oracle_pk);

    coordinator
        .propose_dlc_channel(contract_input, app.info.pubkey)
        .await
        .unwrap();

    let offered_channel = wait_until(Duration::from_secs(30), || async {
        app.process_incoming_messages()?;

        let dlc_channels = app.dlc_manager.get_store().get_offered_channels()?;

        Ok(dlc_channels
            .iter()
            .find(|dlc_channel| dlc_channel.counter_party == coordinator.info.pubkey)
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
            .find(|dlc_channel| dlc_channel.counter_party == app.info.pubkey)
            .cloned())
    })
    .await
    .unwrap();

    let app_signed_channel = wait_until(Duration::from_secs(30), || async {
        app.process_incoming_messages()?;

        let dlc_channels = app.dlc_manager.get_store().get_signed_channels(None)?;

        Ok(dlc_channels
            .iter()
            .find(|dlc_channel| dlc_channel.counter_party == coordinator.info.pubkey)
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

        let app_balance_after_open = app.get_on_chain_balance().unwrap().confirmed;

        // We don't aim to account for transaction fees exactly.
        Ok((app_balance_after_open <= app_balance_before - app_dlc_collateral).then_some(()))
    })
    .await
    .unwrap();

    wait_until(Duration::from_secs(30), || async {
        coordinator.sync_wallets().await.unwrap();

        let coordinator_balance_after_open = coordinator.get_on_chain_balance().unwrap().confirmed;

        // We don't aim to account for transaction fees exactly.
        Ok((coordinator_balance_after_open
            <= coordinator_balance_before - coordinator_dlc_collateral)
            .then_some(()))
    })
    .await
    .unwrap();

    wait_until(Duration::from_secs(30), || async {
        app.dlc_manager.periodic_chain_monitor().unwrap();
        app.dlc_manager.periodic_check().unwrap();

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
            coordinator_signed_channel.channel_id,
            coordinator_dlc_collateral / 2,
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
            .find(|dlc_channel| dlc_channel.counter_party == coordinator.info.pubkey)
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
            .find(|dlc_channel| dlc_channel.counter_party == coordinator.info.pubkey)
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
            .find(|dlc_channel| dlc_channel.counter_party == app.info.pubkey)
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
            .find(|dlc_channel| dlc_channel.counter_party == coordinator.info.pubkey)
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
            .find(|dlc_channel| dlc_channel.counter_party == app.info.pubkey)
            .cloned())
    })
    .await
    .unwrap();
    (
        app,
        coordinator,
        coordinator_signed_channel,
        app_signed_channel,
    )
}
