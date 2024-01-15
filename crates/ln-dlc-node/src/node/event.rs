use anyhow::anyhow;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use dlc_messages::Message;
use tokio::sync::broadcast;
use tokio::sync::broadcast::Receiver;

#[derive(Clone, Debug)]
pub enum NodeEvent {
    Connected { peer: PublicKey },
    SendDlcMessage { peer: PublicKey, msg: Message },
}

#[derive(Clone)]
pub struct NodeEventHandler {
    sender: broadcast::Sender<NodeEvent>,
}

impl Default for NodeEventHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeEventHandler {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(100);

        NodeEventHandler { sender }
    }

    pub fn subscribe(&self) -> Receiver<NodeEvent> {
        self.sender.subscribe()
    }

    pub fn publish(&self, event: NodeEvent) -> Result<()> {
        self.sender.send(event).map_err(|e| anyhow!("{e:#}"))?;

        Ok(())
    }
}
