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
