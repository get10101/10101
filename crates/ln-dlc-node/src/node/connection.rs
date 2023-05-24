use crate::node::Node;
use crate::node::NodeInfo;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use autometrics::autometrics;
use bitcoin::secp256k1::PublicKey;
use futures::Future;
use std::pin::Pin;
use std::time::Duration;

impl<P> Node<P> {
    #[autometrics]
    pub async fn connect(&self, peer: NodeInfo) -> Result<Pin<Box<impl Future<Output = ()>>>> {
        #[allow(clippy::async_yields_async)] // We want to poll this future in a loop elsewhere
        let connection_closed_future = tokio::time::timeout(Duration::from_secs(15), async {
            let mut round = 1;
            loop {
                tracing::debug!(%peer, "Setting up connection");

                if let Some(fut) = lightning_net_tokio::connect_outbound(
                    self.peer_manager.clone(),
                    peer.pubkey,
                    peer.address,
                )
                .await
                {
                    return fut;
                };

                let retry_interval = Duration::from_secs(1) * round;
                tracing::debug!(%peer, ?retry_interval, "Connection setup failed; retrying");
                tokio::time::sleep(retry_interval).await;
                round *= 2;
            }
        })
        .await
        .with_context(|| format!("Failed to connect to peer: {peer}"))?;

        tracing::debug!(%peer, "Connection setup completed");

        let mut connection_closed_future = Box::pin(connection_closed_future);

        tokio::time::timeout(Duration::from_secs(30), async {
            while !self.is_connected(peer.pubkey) {
                if futures::poll!(&mut connection_closed_future).is_ready() {
                    bail!("Peer disconnected before we finished the handshake");
                }

                tracing::debug!(%peer, "Waiting to confirm established connection");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }

            Ok(())
        })
        .await??;

        tracing::info!(%peer, "Connection established");
        Ok(connection_closed_future)
    }

    pub fn is_connected(&self, pubkey: PublicKey) -> bool {
        self.peer_manager
            .get_peer_node_ids()
            .iter()
            .any(|(id, _)| *id == pubkey)
    }
}
