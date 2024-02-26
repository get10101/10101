use crate::ln::common_handlers;
use crate::ln::event_handler::EventSender;
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

/// Event handler for the mobile 10101 app.
// TODO: Move it out of this crate
pub struct AppEventHandler<D: BdkStorage, S: TenTenOneStorage, N: Storage> {
    pub(crate) node: Arc<Node<D, S, N>>,
    pub(crate) event_sender: Option<EventSender>,
}

impl<D: BdkStorage, S: TenTenOneStorage, N: Storage + Sync + Send> AppEventHandler<D, S, N> {
    pub fn new(node: Arc<Node<D, S, N>>, event_sender: Option<EventSender>) -> Self {
        Self { node, event_sender }
    }
}

#[async_trait]
impl<D: BdkStorage, S: TenTenOneStorage + 'static, N: Storage + Sync + Send + 'static>
    EventHandlerTrait for AppEventHandler<D, S, N>
{
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

    fn event_sender(&self) -> &Option<EventSender> {
        &self.event_sender
    }
}
