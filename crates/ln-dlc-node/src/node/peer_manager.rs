use crate::node::Storage;
use crate::storage::TenTenOneStorage;
use crate::PeerManager;
use anyhow::ensure;
use lightning::ln::msgs::NetAddress;

const NODE_COLOR: [u8; 3] = [0; 3];

pub fn broadcast_node_announcement<S: TenTenOneStorage, N: Storage>(
    peer_manager: &PeerManager<S, N>,
    alias: [u8; 32],
    inc_connection_addresses: Vec<NetAddress>,
) {
    let known_peers = peer_manager
        .get_peer_node_ids()
        .iter()
        .map(|(pk, _)| pk.to_string())
        .collect::<Vec<_>>();
    tracing::debug!(
        ?inc_connection_addresses,
        ?known_peers,
        "Broadcasting node announcement"
    );
    peer_manager.broadcast_node_announcement(NODE_COLOR, alias, inc_connection_addresses)
}

pub fn alias_as_bytes(alias: &str) -> anyhow::Result<[u8; 32]> {
    ensure!(
        alias.len() <= 32,
        "Node Alias can not be longer than 32 bytes"
    );

    let mut bytes = [0; 32];
    bytes[..alias.len()].copy_from_slice(alias.as_bytes());

    Ok(bytes)
}
