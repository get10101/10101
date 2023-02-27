use crate::node::Node;
use crate::node::NodeInfo;
use crate::PeerManager;
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
        let connection_closed_future =
            lightning_net_tokio::connect_outbound(peer_manager.clone(), peer.pubkey, peer.address)
                .await
                .context("Failed to connect to counterparty")?;

        let mut connection_closed_future = Box::pin(connection_closed_future);
        while !Self::is_connected(&peer_manager, peer.pubkey) {
            if futures::poll!(&mut connection_closed_future).is_ready() {
                bail!("Peer disconnected before we finished the handshake");
            }

            tracing::debug!(%peer, "Waiting to establish connection");
            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        tracing::info!(%peer, "Connection established");
        Ok(connection_closed_future)
    }

    pub async fn keep_connected(&self, peer: NodeInfo) -> Result<()> {
        // TODO: Let this time out
        let connection_closed_future = loop {
            tracing::debug!(%peer, "Attempting to establish initial connection");

            if let Ok(fut) = Self::connect(self.peer_manager.clone(), peer).await {
                break fut;
            }

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
