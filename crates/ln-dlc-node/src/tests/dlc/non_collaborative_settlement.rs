use crate::node::Node;
use crate::tests::bitcoind::mine;
use crate::tests::dlc::create::create_dlc_channel;
use crate::tests::dlc::create::DlcChannelCreated;
use crate::tests::init_tracing;
use anyhow::Context;
use anyhow::Result;
use std::sync::Arc;

// Related issue: https://github.com/get10101/10101/issues/51.
#[tokio::test]
#[ignore]
async fn given_dlc_channel_present_when_dlc_settled_non_collaboratively_then_sibling_channel_operational(
) {
    init_tracing();

    let app_dlc_collateral = 50_000;
    let coordinator_dlc_collateral = 25_000;

    dlc_non_collaborative_settlement(app_dlc_collateral, coordinator_dlc_collateral)
        .await
        .unwrap();
}
/// Start an app and a coordinator; create an LN channel between them with double the specified
/// amounts; add a DLC channel with the specified amounts; and close the DLC channel giving the
/// coordinator 50% losses.
async fn dlc_non_collaborative_settlement(
    app_dlc_collateral: u64,
    coordinator_dlc_collateral: u64,
) -> Result<(Node, Arc<Node>)> {
    // Arrange

    let DlcChannelCreated {
        coordinator,
        coordinator_balance_channel_creation,
        app,
        app_balance_channel_creation,
        channel_id,
    } = create_dlc_channel(app_dlc_collateral, coordinator_dlc_collateral).await?;

    let coordinator = Arc::new(coordinator);

    tokio::task::spawn_blocking({
        let coordinator = coordinator.clone();
        move || {
            coordinator
                .sub_channel_manager
                .initiate_force_close_sub_channel(&channel_id)
                .unwrap();
        }
    })
    .await
    .unwrap();

    mine(500).await.unwrap();

    tokio::task::spawn_blocking({
        let coordinator = coordinator.clone();
        move || {
            coordinator
                .sub_channel_manager
                .finalize_force_close_sub_channels(&channel_id)
                .unwrap();
        }
    })
    .await
    .unwrap();

    coordinator.sync();
    app.sync();

    let coordinator_off_chain_balance = dbg!(coordinator.get_ldk_balance());
    let coordinator_on_chain_balance = dbg!(coordinator.get_on_chain_balance().unwrap());

    assert_eq!(coordinator_off_chain_balance.available, 0);

    Ok((app, coordinator))
}
