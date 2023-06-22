use anyhow::Result;
use native::api;
use tests_e2e::app::run_app;
use tests_e2e::fund::fund_app_with_faucet;
use tests_e2e::http::init_reqwest;
use tests_e2e::tracing::init_tracing;

#[tokio::test]
#[ignore = "need to be run with 'just e2e' command"]
async fn app_can_be_funded_with_lnd_faucet() -> Result<()> {
    init_tracing();

    let client = init_reqwest();

    // Check whether the coordinator is running
    client
        .get("http://localhost:8000/api/newaddress")
        .send()
        .await
        .expect("Could not send request to coordinator")
        .error_for_status()
        .expect("Coordinator did not return 200 OK");

    let app = run_app().await;

    // this is just to showcase we can retrieve value from a SyncReturn
    let node_id: String = api::get_node_id().0;
    tracing::info!("Node ID: {}", node_id);

    // Unfunded wallet should be empty
    assert_eq!(app.rx.wallet_info().unwrap().balances.on_chain, 0);
    assert_eq!(app.rx.wallet_info().unwrap().balances.lightning, 0);

    let funding_amount = 50_000;
    fund_app_with_faucet(&client, funding_amount).await?;

    assert_eq!(app.rx.wallet_info().unwrap().balances.on_chain, 0);
    assert_eq!(
        app.rx.wallet_info().unwrap().balances.lightning,
        funding_amount
    );
    Ok(())
}
