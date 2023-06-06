use autometrics::autometrics;
use lightning::ln::msgs::NetAddress;
use ln_dlc_node::node::Node;
use ln_dlc_node::node::NodeInfo;
use ln_dlc_node::node::PaymentMap;
use rand::seq::SliceRandom;
use rand::thread_rng;
use std::net::IpAddr;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::spawn_blocking;

#[autometrics]
pub async fn keep_public_channel_peers_connected(
    node: Arc<Node<PaymentMap>>,
    check_interval: Duration,
) {
    loop {
        spawn_blocking({
            let node = node.clone();
            move || reconnect_to_disconnected_public_channel_peers(node)
        })
        .await
        .expect("Failed to spawn blocking task");

        tokio::time::sleep(check_interval).await;
    }
}

fn reconnect_to_disconnected_public_channel_peers(node: Arc<Node<PaymentMap>>) {
    let connected_peers = node
        .peer_manager
        .get_peer_node_ids()
        .into_iter()
        .map(|(peer, _)| peer)
        .collect::<Vec<_>>();

    let channels = node.channel_manager.list_channels();
    let peers_with_public_channel = channels
        .iter()
        .filter_map(|c| c.is_public.then_some(c.counterparty.node_id));

    for peer in peers_with_public_channel.filter(|peer| !connected_peers.contains(peer)) {
        let addresses = match node
            .network_graph
            .read_only()
            .get_addresses(&peer)
            .map(|v| {
                v.into_iter()
                    .filter_map(net_address_to_socket_addr)
                    .collect::<Vec<_>>()
            }) {
            None => {
                tracing::warn!(%peer, "Cannot reconnect to unknown public node");
                continue;
            }
            Some(addresses) if addresses.is_empty() => {
                tracing::warn!(%peer, "Cannot reconnect to public node without known addresses");
                continue;
            }
            Some(addresses) => addresses,
        };

        tokio::spawn({
            let node = node.clone();
            let mut addresses = addresses.clone();
            async move {
                tracing::debug!(%peer, "Establishing connection with public channel peer");

                // We shuffle the addresses so as to not always retry
                addresses.shuffle(&mut thread_rng());

                for address in addresses {
                    let node_info = NodeInfo {
                        pubkey: peer,
                        address,
                    };

                    match node.connect(node_info).await {
                        Ok(connection_closed_future) => {
                            connection_closed_future.await;
                            tracing::debug!(
                                %peer,
                                "Connection lost with public channel peer"
                            );

                            // We return from the future and not just break out of the loop. This is
                            // intentional, as we want to have one task per peer at a time
                            return;
                        }
                        Err(e) => {
                            tracing::trace!(%peer, %address, "Failed to connect to public channel peer: {e:#}")
                        }
                    };
                }

                tracing::warn!(%peer, "Failed to connect to public channel peer on all addresses");
            }
        });
    }
}

fn net_address_to_socket_addr(net_address: NetAddress) -> Option<SocketAddr> {
    match net_address {
        NetAddress::IPv4 { addr, port } => Some(SocketAddr::new(IpAddr::from(addr), port)),
        NetAddress::IPv6 { addr, port } => Some(SocketAddr::new(IpAddr::from(addr), port)),
        // TODO: If we want to be able to connect to peers using Tor, we will have to use a Tor
        // proxy
        NetAddress::OnionV2(_) => None,
        NetAddress::OnionV3 { .. } => None,
        NetAddress::Hostname { .. } => None,
    }
}
