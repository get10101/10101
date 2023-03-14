use crate::config;
use anyhow::Result;
use bdk::bitcoin::secp256k1::SecretKey;
use bdk::bitcoin::secp256k1::SECP256K1;
use futures::TryStreamExt;
use orderbook_commons::Signature;
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
        runtime.spawn(async move {
            let url = format!(
                "ws://{}/api/orderbook/websocket",
                config::get_http_endpoint()
            );

            let pubkey = secret_key.public_key(SECP256K1);
            let authenticate = |msg| {
                let signature = secret_key.sign_ecdsa(msg);
                Signature { pubkey, signature }
            };
            let mut stream = orderbook_client::subscribe_with_authentication(url, &authenticate);

            while let Ok(Some(result)) = stream.try_next().await {
                tracing::info!("Received: {result}");
            }
        });
        Ok(())
    })
}
