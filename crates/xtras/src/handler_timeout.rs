use std::ops::ControlFlow;
use std::time::Duration;
use tokio::time::timeout;
use xtra::Actor;
use xtra::Context;

/// Extension trait which converts a `Context` into a [`TimeoutManager`], which will run the actor
/// with a given handler timeout duration.
pub trait HandlerTimeoutExt<A> {
    /// Wrap the given context in a [`TimeoutManager`]
    fn with_handler_timeout(self, timeout: Duration) -> TimeoutManager<A>;
}

impl<A> HandlerTimeoutExt<A> for Context<A> {
    fn with_handler_timeout(self, timeout: Duration) -> TimeoutManager<A> {
        TimeoutManager { ctx: self, timeout }
    }
}

pub struct TimeoutManager<A> {
    ctx: Context<A>,
    timeout: Duration,
}

impl<A> TimeoutManager<A>
where
    A: Actor,
{
    /// Run the actor with the previously specified handler timeout duration. See
    /// [`HandlerTimeoutExt`] for more.
    pub async fn run(mut self, mut actor: A) -> A::Stop {
        actor.started(&mut self.ctx).await;

        if !self.ctx.running {
            return actor.stopped().await;
        }

        loop {
            let mut fut = self.ctx.tick(self.ctx.next_message().await, &mut actor);
            let span = fut.get_or_create_span().clone();

            match timeout(self.timeout, fut).await {
                Ok(ControlFlow::Continue(())) => (),
                Ok(ControlFlow::Break(())) => break actor.stopped().await,
                Err(_elapsed) => {
                    let _g = span.enter();
                    span.record("interrupted", "timed_out");
                    tracing::warn!(
                        timeout_seconds = self.timeout.as_secs(),
                        "Handler execution timed out"
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use xtra::Handler;

    struct MyActor {
        started: bool,
    }

    #[async_trait::async_trait]
    impl Actor for MyActor {
        type Stop = bool;

        async fn started(&mut self, _ctx: &mut Context<Self>) {
            self.started = true;
        }

        async fn stopped(self) -> Self::Stop {
            self.started
        }
    }

    #[async_trait::async_trait]
    impl Handler<()> for MyActor {
        type Return = ();

        async fn handle(&mut self, _: (), _ctx: &mut Context<Self>) {
            assert!(self.started);
        }
    }

    #[tokio::test]
    async fn started_is_called() {
        let (addr, ctx) = Context::new(None);
        let fut = ctx
            .with_handler_timeout(Duration::from_secs(1))
            .run(MyActor { started: false });
        let _ = addr.send(()).split_receiver().await;
        drop(addr);
        assert!(fut.await);
    }
}
