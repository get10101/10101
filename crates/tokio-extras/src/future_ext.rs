use crate::time::Timeout;
use futures::future::RemoteHandle;
use futures::Future;
use futures::FutureExt as _;
use std::any::Any;
use std::any::TypeId;
use std::time::Duration;
use tracing::Instrument;
use tracing::Span;

pub trait FutureExt: Future + Sized {
    /// Spawn the `Future` of a task in the runtime and return a
    /// `RemoteHandle` to it. The task will be stopped when the handle
    /// is dropped.
    fn spawn_with_handle(self) -> RemoteHandle<Self::Output>
    where
        Self: Send + Any + 'static,
        Self::Output: Send;

    /// Limit the future's time of execution to a certain duration, cancelling it and returning
    /// an error if time runs out. This is instrumented, unlike `tokio::time::timeout`. The
    /// `child_span` function constructs the span for the child future from the span of the parent
    /// (timeout) future.
    fn timeout(self, duration: Duration, child_span: fn() -> Span) -> Timeout<Self>;
}

impl<F> FutureExt for F
where
    F: Future,
{
    fn spawn_with_handle(self) -> RemoteHandle<F::Output>
    where
        Self: Send + Any + 'static,
        F::Output: Send,
    {
        debug_assert!(
            TypeId::of::<RemoteHandle<Self::Output>>() != self.type_id(),
            "RemoteHandle<()> is a handle to already spawned task",
        );

        let span = tracing::trace_span!(parent: Span::none(), "Spawned task");
        span.follows_from(Span::current());

        let (future, handle) = self.instrument(span).remote_handle();
        // we want to disallow calls to tokio::spawn outside FutureExt
        #[allow(clippy::disallowed_methods)]
        tokio::spawn(future);
        handle
    }

    fn timeout(self, duration: Duration, child_span: fn() -> Span) -> Timeout<F> {
        crate::time::timeout(duration, self, child_span)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::sleep;
    use std::panic;
    use std::time::Duration;

    #[tokio::test]
    async fn spawning_a_regular_future_does_not_panic() {
        let result = panic::catch_unwind(|| {
            let _handle = sleep(Duration::from_secs(2)).spawn_with_handle();
        });
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn panics_when_called_spawn_with_handle_on_remote_handle() {
        let result = panic::catch_unwind(|| {
            let handle = sleep(Duration::from_secs(2)).spawn_with_handle();
            let _handle_to_a_handle = handle.spawn_with_handle();
        });

        if cfg!(debug_assertions) {
            assert!(
                result.is_err(),
                "Spawning a remote handle into a separate task should panic_in_debug_mode"
            );
        } else {
            assert!(result.is_ok(), "Do not panic in release mode");
        }
    }
}
