use async_trait::async_trait;
use tracing::instrument;
use xtra::message_channel::MessageChannel;
use xtra::refcount::RefCounter;

/// Arbitrarily high number allowing flexibility of prioritising messages in actors
const INTERNAL_MSG_PRIORITY: u32 = 100;

#[async_trait]
pub trait SendAsyncNext<M: Send + 'static, R> {
    /// Dispatch a prioritised message to an actor without waiting for it to handle it.
    ///
    /// If the actor is not connected, it logs a warning.
    async fn send_async_next(&self, msg: M);
}

#[async_trait]
impl<A, M> SendAsyncNext<M, ()> for xtra::Address<A>
where
    A: xtra::Handler<M, Return = ()>,
    M: Send + 'static,
{
    #[instrument(skip_all)]
    async fn send_async_next(&self, msg: M) {
        if !self.is_connected() {
            tracing::warn!("Actor not connected when sending message");
        }
        let _ = self
            .send(msg)
            .priority(INTERNAL_MSG_PRIORITY)
            .split_receiver()
            .await;
    }
}

#[async_trait]
impl<M, Rc: RefCounter> SendAsyncNext<M, ()> for MessageChannel<M, (), Rc>
where
    M: Send + 'static,
{
    #[instrument(skip(msg))]
    async fn send_async_next(&self, msg: M) {
        if !self.is_connected() {
            tracing::warn!("Actor not connected when sending message");
        }
        let _ = self
            .send(msg)
            .priority(INTERNAL_MSG_PRIORITY)
            .split_receiver()
            .await;
    }
}
