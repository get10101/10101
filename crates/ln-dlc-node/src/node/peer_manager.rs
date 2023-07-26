use anyhow::ensure;
use lightning::ln::msgs::NetAddress;

use crate::PeerManager;

const NODE_COLOR: [u8; 3] = [0; 3];

pub(crate) fn broadcast_node_announcement(
    peer_manager: &PeerManager,
    alias: [u8; 32],
    inc_connection_addresses: Vec<NetAddress>,
) {
    tracing::debug!(?inc_connection_addresses, "Broadcasting node announcement");

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
