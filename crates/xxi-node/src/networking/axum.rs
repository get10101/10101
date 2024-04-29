use crate::networking::DynamicSocketDescriptor;
use anyhow::Context;
use axum::extract::ws::Message;
use axum::extract::ws::WebSocket;
use futures::future::Either;
use futures::StreamExt;
use lightning::ln::peer_handler;
use lightning::ln::peer_handler::APeerManager;
use std::future;
use std::hash::Hash;
use std::hash::Hasher;
use std::net::SocketAddr;
use std::ops::ControlFlow;
use std::ops::Deref;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use tokio::sync::mpsc;
use tokio::sync::mpsc::UnboundedReceiver;
use tracing::error;

static ID_COUNTER: AtomicU64 = AtomicU64::new(0);

pub async fn setup_inbound<PM: Deref + 'static + Send + Sync + Clone>(
    peer_manager: PM,
    mut ws: WebSocket,
    remote: SocketAddr,
) where
    PM::Target: APeerManager<Descriptor = DynamicSocketDescriptor>,
{
    let (task_tx, mut task_rx) = mpsc::unbounded_channel();
    let mut descriptor = DynamicSocketDescriptor::Axum(SocketDescriptor {
        tx: task_tx,
        id: ID_COUNTER.fetch_add(1, Ordering::AcqRel),
    });

    if peer_manager
        .as_ref()
        .new_inbound_connection(descriptor.clone(), Some(remote.into()))
        .is_ok()
    {
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
                    peer_manager.as_ref().socket_disconnected(&descriptor);
                    peer_manager.as_ref().process_events();
                    break;
                }
            }
        }

        let _ = ws.close().await;
    }
}

async fn process_messages<PM>(
    peer_manager: &PM,
    task_rx: &mut UnboundedReceiver<BgTaskMessage>,
    ws: &mut WebSocket,
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
                return Ok(ControlFlow::Break(()))
            },
        },
        ws_msg = ws_next => {
            let data = ws_msg.context("WS returned no data")??.into_data();
            if let Ok(true) = peer_manager.as_ref().read_event(descriptor, &data) {
                *emit_read_events = false; // Pause read events
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
        // See the TODO in tungstenite.rs
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
