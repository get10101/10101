use crate::node::Node;
use crate::tests::dlc::collaborative_settlement::collaborative_settlement;
use crate::tests::dlc::create::create_dlc_channel_with_nodes;
use crate::tests::init_tracing;

#[tokio::test(flavor = "multi_thread", worker_threads = 10)]
#[ignore]
async fn multiple_dlc_positions() {
    init_tracing();

    let app_dlc_collateral = 50_000;
    let coordinator_dlc_collateral = 25_000;

    let app = Node::start_test_app("app").await.unwrap();
    let coordinator = Node::start_test_coordinator("coordinator").await.unwrap();

    for n in 1..5 {
        tracing::info!("-----------------__> Opening {n}. DLC Position.");

        let (coordinator_balance_channel_creation, app_balance_channel_creation, channel_details) =
            create_dlc_channel_with_nodes(
                app_dlc_collateral,
                coordinator_dlc_collateral,
                &app,
                &coordinator,
            )
            .await
            .unwrap();

        tracing::info!("-----------------__> Closing {n}. DLC Position.");

        collaborative_settlement(
            coordinator_dlc_collateral,
            &coordinator,
            coordinator_balance_channel_creation,
            &app,
            app_balance_channel_creation,
            &channel_details,
        )
        .await
        .unwrap();

        tracing::info!("-----------------__> Done {n}. DLC Position.");
    }
}
