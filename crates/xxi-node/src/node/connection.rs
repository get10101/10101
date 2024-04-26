use crate::bitcoin_conversion::to_secp_pk_29;
use crate::bitcoin_conversion::to_secp_pk_30;
use crate::networking;
use crate::node::event::NodeEvent;
use crate::node::event::NodeEventHandler;
use crate::node::Node;
use crate::node::NodeInfo;
use crate::node::Storage;
use crate::on_chain_wallet::BdkStorage;
use crate::storage::TenTenOneStorage;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use futures::Future;
use lightning::events::OnionMessageProvider;
use lightning::ln::features::InitFeatures;
use lightning::ln::features::NodeFeatures;
use lightning::ln::msgs;
use lightning::ln::msgs::OnionMessage;
use lightning::ln::msgs::OnionMessageHandler;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

pub struct TenTenOneOnionMessageHandler {
    handler: Arc<NodeEventHandler>,
}

impl TenTenOneOnionMessageHandler {
    pub fn new(handler: Arc<NodeEventHandler>) -> Self {
        TenTenOneOnionMessageHandler { handler }
    }
}

/// Copied from the IgnoringMessageHandler
impl OnionMessageProvider for TenTenOneOnionMessageHandler {
    fn next_onion_message_for_peer(
        &self,
        _peer_node_id: bitcoin_old::secp256k1::PublicKey,
    ) -> Option<OnionMessage> {
        None
    }
}

/// Copied primarily from the IgnoringMessageHandler. Using the peer_connected hook to get notified
/// once a peer successfully connected. (This also includes that the Init Message has been processed
/// and the connection is ready to use).
impl OnionMessageHandler for TenTenOneOnionMessageHandler {
    fn handle_onion_message(
        &self,
        _their_node_id: &bitcoin_old::secp256k1::PublicKey,
        _msg: &OnionMessage,
    ) {
    }
    fn peer_connected(
        &self,
        their_node_id: &bitcoin_old::secp256k1::PublicKey,
        _init: &msgs::Init,
        inbound: bool,
    ) -> Result<(), ()> {
        tracing::info!(%their_node_id, inbound, "Peer connected!");

        self.handler.publish(NodeEvent::Connected {
            peer: to_secp_pk_30(*their_node_id),
        });

        Ok(())
    }
    fn peer_disconnected(&self, _their_node_id: &bitcoin_old::secp256k1::PublicKey) {}
    fn provided_node_features(&self) -> NodeFeatures {
        NodeFeatures::empty()
    }
    fn provided_init_features(
        &self,
        _their_node_id: &bitcoin_old::secp256k1::PublicKey,
    ) -> InitFeatures {
        InitFeatures::empty()
    }
}

impl<D: BdkStorage, S: TenTenOneStorage + 'static, N: Storage + Sync + Send + 'static>
    Node<D, S, N>
{
    /// Establish a connection with a peer.
    ///
    /// # Returns
    ///
    /// If successful, a [`Future`] is returned which will be ready once the connection has been
    /// _lost_. This is meant to be used by the caller to know when to initiate a reconnect if they
    /// want to keep the connection alive.
    pub async fn connect(&self, peer: NodeInfo) -> Result<Pin<Box<impl Future<Output = ()>>>> {
        #[allow(clippy::async_yields_async)] // We want to poll this future in a loop elsewhere
        let connection_closed_future = tokio::time::timeout(Duration::from_secs(15), async {
            let mut round = 1;
            loop {
                tracing::debug!(%peer, "Setting up connection");

                if let Some(fut) =
                    networking::connect_outbound(self.peer_manager.clone(), peer).await
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

    /// Establish a one-time connection with a peer.
    ///
    /// The caller is not interested in knowing if the connection is ever lost. If the caller does
    /// care about that, they should use `connect` instead.
    pub async fn connect_once(&self, peer: NodeInfo) -> Result<()> {
        let fut = self.connect(peer).await?;

        // The caller does not care if the connection is dropped eventually.
        drop(fut);

        Ok(())
    }

    pub fn is_connected(&self, pubkey: PublicKey) -> bool {
        self.peer_manager
            .get_peer_node_ids()
            .iter()
            .any(|(id, _)| *id == to_secp_pk_29(pubkey))
    }
}
