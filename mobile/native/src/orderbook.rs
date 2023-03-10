use anyhow::Result;
use bdk::bitcoin::secp256k1::SecretKey;
use futures::TryStreamExt;
use orderbook_client::Credentials;
use state::Storage;
use tokio::runtime::Runtime;

fn runtime() -> Result<&'static Runtime> {
    static RUNTIME: Storage<Runtime> = Storage::new();

    if RUNTIME.try_get().is_none() {
        let runtime = Runtime::new()?;
        RUNTIME.set(runtime);
    }

    Ok(RUNTIME.get())
}

pub fn subscribe(secret_key: SecretKey) -> Result<()> {
    let runtime = runtime()?;

    runtime.block_on(async move {
        let _ = runtime
            .spawn(async move {
                let url = "ws://localhost:8000/api/orderbook/websocket".to_string();

                let mut stream = orderbook_client::subscribe_with_authentication(
                    url,
                    Credentials { secret_key },
                );

                while let Ok(Some(result)) = stream.try_next().await {
                    tracing::info!("Received: {result}");
                }
            })
            .await;
        Ok(())
    })
}
