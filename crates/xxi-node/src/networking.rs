use crate::node::NodeInfo;
use futures::future;
use lightning::ln::peer_handler::APeerManager;
use lightning::ln::peer_handler::SocketDescriptor;
use std::future::Future;
use std::ops::Deref;
use tracing::debug;

#[cfg(feature = "ln_net_axum_ws")]
pub mod axum;
#[cfg(feature = "ln_net_tcp")]
pub mod tcp;
#[cfg(feature = "ln_net_ws")]
mod tungstenite;

#[allow(clippy::diverging_sub_expression, unused_variables, unreachable_code)] // From the panic!() below
pub async fn connect_outbound<PM: Deref + 'static + Send + Sync + Clone>(
    peer_manager: PM,
    peer: NodeInfo,
) -> Option<impl Future<Output = ()>>
where
    PM::Target: APeerManager<Descriptor = DynamicSocketDescriptor>,
{
    if peer.is_ws {
        debug!("Connecting over WS");

        #[cfg(not(feature = "ln_net_ws"))]
        let ws: Option<future::Either<future::Ready<()>, _>> =
            panic!("Cannot connect outbound over WS when ln_net_ws is not enabled");

        #[cfg(feature = "ln_net_ws")]
        let ws = tungstenite::connect_outbound(peer_manager, peer)
            .await
            .map(future::Either::Left);

        ws
    } else {
        debug!("Connecting over TCP");

        #[cfg(not(feature = "ln_net_tcp"))]
        let tcp: Option<future::Either<_, future::Ready<()>>> =
            panic!("Cannot connect outbound over TCP when ln_net_tcp is not enabled");

        #[cfg(feature = "ln_net_tcp")]
        let tcp = tcp::connect_outbound(peer_manager, peer.pubkey, peer.address)
            .await
            .map(future::Either::Right);

        tcp
    }
}

/// A dynamic socket descriptor that could either be over WASM (JS) websockets, TCP sockets
/// (lightning_net_tokio), or Axum websockets.
#[derive(Hash, Clone, Eq, PartialEq)]
pub enum DynamicSocketDescriptor {
    #[cfg(feature = "ln_net_tcp")]
    Tcp(tcp::SocketDescriptor),
    #[cfg(feature = "ln_net_axum_ws")]
    Axum(axum::SocketDescriptor),
    #[cfg(feature = "ln_net_ws")]
    Tungstenite(tungstenite::SocketDescriptor),
}

impl SocketDescriptor for DynamicSocketDescriptor {
    fn send_data(&mut self, data: &[u8], resume_read: bool) -> usize {
        match self {
            #[cfg(feature = "ln_net_tcp")]
            DynamicSocketDescriptor::Tcp(sock) => sock.send_data(data, resume_read),
            #[cfg(feature = "ln_net_axum_ws")]
            DynamicSocketDescriptor::Axum(sock) => sock.send_data(data, resume_read),
            #[cfg(feature = "ln_net_ws")]
            DynamicSocketDescriptor::Tungstenite(sock) => sock.send_data(data, resume_read),
        }
    }

    fn disconnect_socket(&mut self) {
        match self {
            #[cfg(feature = "ln_net_tcp")]
            DynamicSocketDescriptor::Tcp(sock) => sock.disconnect_socket(),
            #[cfg(feature = "ln_net_axum_ws")]
            DynamicSocketDescriptor::Axum(sock) => sock.disconnect_socket(),
            #[cfg(feature = "ln_net_ws")]
            DynamicSocketDescriptor::Tungstenite(sock) => sock.disconnect_socket(),
        }
    }
}
