use futures::FutureExt;
use std::fmt::Display;
use std::future::Future;
use tracing::Instrument;
use tracing::Span;
use xtra::refcount::RefCounter;
use xtra::Address;

/// Spawn a task that is scoped to the lifetime of the given address.
pub fn spawn<A, Rc, F>(addr: &Address<A, Rc>, fut: F)
where
    Rc: RefCounter,
    F: Future + Send + 'static,
{
    let span = tracing::trace_span!(parent: Span::none(), "Spawned task");
    span.follows_from(Span::current());

    #[allow(clippy::disallowed_methods)]
    tokio::spawn(xtra::scoped(addr, fut.map(|_| ())).instrument(span));
}

/// Spawn a fallible task that is scoped to the lifetime of the given address.
pub fn spawn_fallible<A, Rc, Task, Ok, Err, Fn, FnFut>(
    addr: &Address<A, Rc>,
    fut: Task,
    handle_err: Fn,
) where
    Rc: RefCounter,
    Task: Future<Output = Result<Ok, Err>> + Send + 'static,
    Fn: FnOnce(Err) -> FnFut + Send + 'static,
    FnFut: Future + Send,
    Ok: Send,
    Err: Send + Display,
{
    let span = tracing::trace_span!(parent: Span::none(), "Spawned task");
    span.follows_from(Span::current());

    let task = async {
        if let Err(err) = fut.await {
            let span = tracing::error_span!("fallible task handle_error", %err);
            handle_err(err).instrument(span).await;
        }
    };

    #[allow(clippy::disallowed_methods)]
    tokio::spawn(xtra::scoped(addr, task).instrument(span));
}
