use crate::bitcoin_conversion::to_secp_pk_29;
use crate::networking::DynamicSocketDescriptor;
use crate::node::NodeInfo;
use anyhow::Context;
use futures::future::Either;
use futures::SinkExt;
use futures::StreamExt;
use lightning::ln::peer_handler;
use lightning::ln::peer_handler::APeerManager;
use std::future;
use std::future::Future;
use std::hash::Hash;
use std::hash::Hasher;
use std::ops::ControlFlow;
use std::ops::Deref;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use tokio::sync::mpsc;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio_tungstenite_wasm::Message;
use tokio_tungstenite_wasm::WebSocketStream;
use tracing::error;

static ID_COUNTER: AtomicU64 = AtomicU64::new(0);

pub async fn connect_outbound<PM>(
    peer_manager: PM,
    node_info: NodeInfo,
) -> Option<impl Future<Output = ()>>
where
    PM: Deref + 'static + Send + Sync + Clone,
    PM::Target: APeerManager<Descriptor = DynamicSocketDescriptor>,
{
    let url = &format!(
        "ws://{}:{}",
        node_info.address.ip(),
        node_info.address.port()
    );
    let mut ws = tokio_tungstenite_wasm::connect(url)
        .await
        .map_err(|err| error!("error connecting to peer over websocket: {err:#?}"))
        .ok()?;

    let (task_tx, mut task_rx) = mpsc::unbounded_channel();
    let mut descriptor = DynamicSocketDescriptor::Tungstenite(SocketDescriptor {
        tx: task_tx,
        id: ID_COUNTER.fetch_add(1, Ordering::AcqRel),
    });

    if let Ok(initial_send) = peer_manager.as_ref().new_outbound_connection(
        to_secp_pk_29(node_info.pubkey),
        descriptor.clone(),
        Some(node_info.address.into()),
    ) {
        ws.send(Message::Binary(initial_send))
            .await
            .map_err(|err| error!("error sending initial data over websocket: {err:#?}"))
            .ok()?;

        Some(async move {
            let mut emit_read_events = true;
            loop {
                match process_messages(
                    &peer_manager,
                    &mut task_rx,
                    &mut ws,
                    &mut descriptor,
                    &mut emit_read_events,
                )
                .await
                {
                    Ok(ControlFlow::Break(())) => break,
                    Ok(ControlFlow::Continue(())) => (),
                    Err(err) => {
                        error!("Disconnecting websocket with error: {err}");
                        let _ = ws.close().await;
                        peer_manager.as_ref().socket_disconnected(&descriptor);
                        peer_manager.as_ref().process_events();
                        break;
                    }
                }
            }
        })
    } else {
        None
    }
}

async fn process_messages<PM>(
    peer_manager: &PM,
    task_rx: &mut UnboundedReceiver<BgTaskMessage>,
    ws: &mut WebSocketStream,
    descriptor: &mut DynamicSocketDescriptor,
    emit_read_events: &mut bool,
) -> Result<ControlFlow<()>, anyhow::Error>
where
    PM: Deref + 'static + Send + Sync + Clone,
    PM::Target: APeerManager<Descriptor = DynamicSocketDescriptor>,
{
    let ws_next = if *emit_read_events {
        Either::Left(ws.next())
    } else {
        Either::Right(future::pending())
    };

    tokio::select! {
        task_msg = task_rx.recv() => match task_msg.context("rust-lightning SocketDescriptor dropped")? {
            BgTaskMessage::SendData { data, resume_read } => {
                if resume_read {
                    *emit_read_events = true;
                }

                ws.send(Message::Binary(data)).await?;
            },
            BgTaskMessage::Close => {
                let _ = ws.close().await;
                return Ok(ControlFlow::Break(()))
            },
        },
        ws_msg = ws_next => {
            let data = ws_msg.context("WS returned no data")??.into_data();
            if let Ok(true) = peer_manager.as_ref().read_event(descriptor, &data) {
                *emit_read_events = false; // Pause reading
            }

            peer_manager.as_ref().process_events();
        }
    }

    Ok(ControlFlow::Continue(()))
}

enum BgTaskMessage {
    SendData { data: Vec<u8>, resume_read: bool },
    Close,
}

#[derive(Clone)]
pub struct SocketDescriptor {
    tx: mpsc::UnboundedSender<BgTaskMessage>,
    id: u64,
}

impl Eq for SocketDescriptor {}
impl PartialEq for SocketDescriptor {
    fn eq(&self, o: &Self) -> bool {
        self.id == o.id
    }
}

impl Hash for SocketDescriptor {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl peer_handler::SocketDescriptor for SocketDescriptor {
    fn send_data(&mut self, data: &[u8], resume_read: bool) -> usize {
        // TODO(ws):
        // This isn't so great as we should be waiting for this to be sent before returning the
        // amount of data sent (which implies that the send operation is done). This is so that the
        // backpressure stuff works properly
        //
        // It's a little more complicated than it may seem. If we don't send all the data when
        // calling send_data then there is a function we need to call of the peer manager
        // which results in send_data being called again. At first glance you'd think you
        // can just make the 2nd a no-op, but there could be more data that's waiting to be
        // sent by then. Therefore, we need to keep track of how much we promised to send
        // and how much extra must be sent.
        let _ = self.tx.send(BgTaskMessage::SendData {
            data: data.to_vec(),
            resume_read,
        });
        data.len()
    }

    fn disconnect_socket(&mut self) {
        let _ = self.tx.send(BgTaskMessage::Close);
    }
}
