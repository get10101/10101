use super::common_handlers;
use super::event_handler::EventSender;
use crate::node::Node;
use crate::node::Storage;
use crate::on_chain_wallet::BdkStorage;
use crate::storage::TenTenOneStorage;
use crate::EventHandlerTrait;
use anyhow::bail;
use anyhow::Result;
use async_trait::async_trait;
use lightning::events::Event;
use std::sync::Arc;

/// Event handler for the coordinator node.
// TODO: Move it out of this crate
pub struct CoordinatorEventHandler<D: BdkStorage, S: TenTenOneStorage, N: Storage> {
    pub(crate) node: Arc<Node<D, S, N>>,
    pub(crate) event_sender: Option<EventSender>,
}

impl<D: BdkStorage, S: TenTenOneStorage, N: Storage> CoordinatorEventHandler<D, S, N> {
    pub fn new(node: Arc<Node<D, S, N>>, event_sender: Option<EventSender>) -> Self {
        Self { node, event_sender }
    }
}

#[async_trait]
impl<D: BdkStorage, S: TenTenOneStorage + 'static, N: Storage + Send + Sync + 'static>
    EventHandlerTrait for CoordinatorEventHandler<D, S, N>
{
    fn event_sender(&self) -> &Option<EventSender> {
        &self.event_sender
    }

    async fn match_event(&self, event: Event) -> Result<()> {
        match event {
            Event::SpendableOutputs {
                outputs,
                channel_id: _,
            } => {
                // TODO(holzeis): Update shadow channel to store the commitment transaction closing
                // the channel.
                common_handlers::handle_spendable_outputs(&self.node, outputs)?;
            }
            _ => {
                bail!("Unhandled event");
            }
        };

        Ok(())
    }
}
