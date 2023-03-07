use anyhow::Result;
use bitmex_stream::Credentials;
use bitmex_stream::Network;
use futures::TryStreamExt;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info,bitmex_stream=trace")
        .init();

    let mut stream = bitmex_stream::subscribe_with_credentials(
        ["execution".to_owned()],
        Network::Testnet,
        Credentials {
            api_key: "some_api_key".to_string(),
            secret: "some_secret".to_string(),
        },
    );

    while let Some(result) = stream.try_next().await? {
        tracing::info!("{result}");
    }

    Ok(())
}
