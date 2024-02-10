use anyhow::Result;
use tests_e2e::setup::TestSetup;

#[tokio::test(flavor = "multi_thread")]
#[ignore = "need to be run with 'just e2e' command"]
async fn app_can_be_funded_with_bitcoind() -> Result<()> {
    TestSetup::new_after_funding().await;

    Ok(())
}
