use crate::bitcoin_conversion::to_secp_pk_29;
use crate::node::dlc_channel::estimated_dlc_channel_fee_reserve;
use crate::node::event::NodeEvent;
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
use dlc_manager::channel::signed_channel::SignedChannelState;
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

    coordinator
        .event_handler
        .publish(NodeEvent::SendLastDlcMessage {
            peer: app.info.pubkey,
        });

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
async fn can_open_and_force_close_settled_channel() {
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
        let dlc_channels = coordinator
            .dlc_manager
            .get_store()
            .get_signed_channels(None)?;

        Ok(dlc_channels
            .iter()
            .find(|dlc_channel| {
                dlc_channel.counter_party == to_secp_pk_29(app.info.pubkey)
                    && matches!(dlc_channel.state, SignedChannelState::SettledClosing { .. })
            })
            .cloned())
    })
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn funding_transaction_pays_expected_fees() {
    init_tracing();

    // Arrange

    let app_dlc_collateral = Amount::from_sat(10_000);
    let coordinator_dlc_collateral = Amount::from_sat(10_000);

    let fee_rate_sats_per_vb = 2;

    // Give enough funds to app and coordinator so that each party can have their own change output.
    // This is not currently enforced by `rust-dlc`, but it will be in the near future:
    // https://github.com/p2pderivatives/rust-dlc/pull/152.
    let (app, _running_app) = start_and_fund_app(app_dlc_collateral * 2, 1).await;
    let (coordinator, _running_coordinator) =
        start_and_fund_coordinator(app_dlc_collateral * 2, 1).await;

    // Act

    let (app_signed_channel, _) = open_channel_and_position_and_settle_position(
        app.clone(),
        coordinator.clone(),
        app_dlc_collateral,
        coordinator_dlc_collateral,
        Some(fee_rate_sats_per_vb),
    )
    .await;

    // Assert

    let fund_tx_outputs_amount = app_signed_channel
        .fund_tx
        .output
        .iter()
        .fold(Amount::ZERO, |acc, output| {
            acc + Amount::from_sat(output.value)
        });

    let fund_tx_inputs_amount = Amount::from_sat(
        app_signed_channel.own_params.input_amount + app_signed_channel.counter_params.input_amount,
    );

    let fund_tx_fee = fund_tx_inputs_amount - fund_tx_outputs_amount;

    let fund_tx_weight_wu = app_signed_channel.fund_tx.weight();
    let fund_tx_weight_vb = (fund_tx_weight_wu / 4) as u64;

    let fund_tx_fee_rate_sats_per_vb = fund_tx_fee.to_sat() / fund_tx_weight_vb;

    assert_eq!(fund_tx_fee_rate_sats_per_vb, fee_rate_sats_per_vb);
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn dlc_channel_includes_expected_fee_reserve() {
    init_tracing();

    let app_dlc_collateral = Amount::from_sat(10_000);
    let coordinator_dlc_collateral = Amount::from_sat(10_000);

    // We must fix the fee rate so that we can predict how many sats `rust-dlc` will allocate
    // for transaction fees.
    let fee_rate_sats_per_vb = 2;

    let total_fee_reserve = estimated_dlc_channel_fee_reserve(fee_rate_sats_per_vb as f64);

    let expected_fund_output_amount =
        app_dlc_collateral + coordinator_dlc_collateral + total_fee_reserve;

    let (app, _running_app) = start_and_fund_app(app_dlc_collateral * 2, 1).await;
    let (coordinator, _running_coordinator) =
        start_and_fund_coordinator(coordinator_dlc_collateral * 2, 1).await;

    let (app_signed_channel, _) = open_channel_and_position_and_settle_position(
        app.clone(),
        coordinator.clone(),
        app_dlc_collateral,
        coordinator_dlc_collateral,
        Some(fee_rate_sats_per_vb),
    )
    .await;

    let fund_output_vout = app_signed_channel.fund_output_index;
    let fund_output_amount = &app_signed_channel.fund_tx.output[fund_output_vout].value;

    // We cannot easily assert equality because both `rust-dlc` and us have to round in several
    // spots.
    let epsilon = *fund_output_amount as i64 - expected_fund_output_amount.to_sat() as i64;

    assert!(
        epsilon.abs() < 5,
        "Error out of bounds: actual {fund_output_amount} != {}",
        expected_fund_output_amount.to_sat()
    );
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

    let (app_signed_channel, coordinator_signed_channel) =
        open_channel_and_position_and_settle_position(
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

async fn open_channel_and_position_and_settle_position(
    app: Arc<Node<on_chain_wallet::InMemoryStorage, TenTenOneInMemoryStorage, InMemoryStore>>,
    coordinator: Arc<
        Node<on_chain_wallet::InMemoryStorage, TenTenOneInMemoryStorage, InMemoryStore>,
    >,
    app_dlc_collateral: Amount,
    coordinator_dlc_collateral: Amount,
    fee_rate_sats_per_vbyte: Option<u64>,
) -> (SignedChannel, SignedChannel) {
    app.connect_once(coordinator.info).await.unwrap();

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

    coordinator
        .event_handler
        .publish(NodeEvent::SendLastDlcMessage {
            peer: app.info.pubkey,
        });

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

    mine(dlc_manager::manager::NB_CONFIRMATIONS as u16)
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

    coordinator
        .event_handler
        .publish(NodeEvent::SendLastDlcMessage {
            peer: app.info.pubkey,
        });

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

    app.event_handler.publish(NodeEvent::SendLastDlcMessage {
        peer: coordinator.info.pubkey,
    });

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
