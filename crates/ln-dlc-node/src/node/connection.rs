use crate::node::Node;
use crate::node::NodeInfo;
use crate::PeerManager;
use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use futures::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

impl Node {
    async fn connect(
        peer_manager: Arc<PeerManager>,
        peer: NodeInfo,
    ) -> Result<Pin<Box<impl Future<Output = ()>>>> {
        #[allow(clippy::async_yields_async)] // We want to poll this future in a loop elsewhere
        let connection_closed_future = tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                if let Some(fut) = lightning_net_tokio::connect_outbound(
                    peer_manager.clone(),
                    peer.pubkey,
                    peer.address,
                )
                .await
                {
                    return fut;
                };

                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        })
        .await
        .with_context(|| format!("Failed to connect to peer: {peer}"))?;

        tracing::debug!(%peer, "Connection setup completed");

        let mut connection_closed_future = Box::pin(connection_closed_future);

        tokio::time::timeout(Duration::from_secs(30), async {
            while !Self::is_connected(&peer_manager, peer.pubkey) {
                if futures::poll!(&mut connection_closed_future).is_ready() {
                    bail!("Peer disconnected before we finished the handshake");
                }

                tracing::debug!(%peer, "Waiting to confirm established connection");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }

            Ok(())
        })
        .await
        .map_err(|e| anyhow!(e.to_string()))??;

        tracing::info!(%peer, "Connection established");
        Ok(connection_closed_future)
    }

    pub async fn connect_to_peer(&self, peer: NodeInfo) -> Result<()> {
        Self::connect(self.peer_manager.clone(), peer).await?;
        Ok(())
    }

    pub async fn keep_connected(&self, peer: NodeInfo) -> Result<()> {
        let connection_closed_future = loop {
            tracing::debug!(%peer, "Attempting to establish initial connection");

            let error = match Self::connect(self.peer_manager.clone(), peer).await {
                Ok(fut) => break fut,
                Err(e) => e,
            };

            tracing::warn!(%peer, "Failed to establish initial connection: {error:#}");

            tokio::time::sleep(Duration::from_secs(1)).await;
        };

        let peer_manager = self.peer_manager.clone();
        tokio::spawn({
            async move {
                let mut connection_closed_future = connection_closed_future;

                loop {
                    tracing::debug!(%peer, "Keeping connection alive");

                    connection_closed_future.await;
                    tracing::debug!(%peer, "Connection lost");

                    loop {
                        tracing::debug!(%peer, "Attempting to reconnect");

                        if let Ok(fut) = Self::connect(peer_manager.clone(), peer).await {
                            connection_closed_future = fut;
                            break;
                        }

                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                }
            }
        });

        Ok(())
    }

    fn is_connected(peer_manager: &Arc<PeerManager>, pubkey: PublicKey) -> bool {
        peer_manager
            .get_peer_node_ids()
            .iter()
            .any(|id| *id == pubkey)
    }
}
