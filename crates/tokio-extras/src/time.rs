use std::future::Future;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use std::time::Duration;
use tokio::time::error::Elapsed;
use tokio::time::Timeout as TokioTimeout;
use tracing::field;
use tracing::Span;

#[tracing::instrument(name = "Sleep")]
pub async fn sleep(duration: Duration) {
    #[allow(clippy::disallowed_methods)]
    tokio::time::sleep(duration).await
}

/// Sleep without instrumenting the span
pub async fn sleep_silent(duration: Duration) {
    #[allow(clippy::disallowed_methods)]
    tokio::time::sleep(duration).await
}

/// Limit the future's time of execution to a certain duration, cancelling it and returning
/// an error if time runs out. This is instrumented, unlike `tokio::time::timeout`. The
/// `child_span` function constructs the span for the child future from the span of the parent
/// (timeout) future.
pub fn timeout<F>(duration: Duration, fut: F, child_span: fn() -> Span) -> Timeout<F>
where
    F: Future,
{
    #[allow(clippy::disallowed_methods)]
    Timeout {
        fut: tokio::time::timeout(duration, fut),
        instrumentation: Instrumentation::New {
            child_span,
            duration,
        },
    }
}

/// Child-span constructor to pass to [`timeout`] or [`crate::future_ext::FutureExt::timeout`]
/// if the future being timed out is already instrumented.
pub fn already_instrumented() -> Span {
    Span::current()
}

pin_project_lite::pin_project! {
    pub struct Timeout<F> {
        #[pin]
        fut: TokioTimeout<F>,
        instrumentation: Instrumentation,
    }
}

enum Instrumentation {
    New {
        child_span: fn() -> Span,
        duration: Duration,
    },
    Entered {
        parent: Span,
        child: Span,
    },
}

impl<F> Future for Timeout<F>
where
    F: Future,
{
    type Output = Result<F::Output, Elapsed>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        let (poll, parent) = match this.instrumentation {
            Instrumentation::New {
                child_span,
                duration,
            } => {
                let parent = tracing::debug_span!(
                    "Future with timeout",
                    timeout_secs = duration.as_secs(),
                    timed_out = field::Empty,
                )
                .or_current();
                let child = parent.in_scope(child_span).or_current();

                let poll = child.in_scope(|| this.fut.poll(cx));

                *this.instrumentation = Instrumentation::Entered {
                    parent: parent.clone(),
                    child,
                };

                (poll, parent)
            }
            Instrumentation::Entered { parent, child } => {
                (child.in_scope(|| this.fut.poll(cx)), parent.clone())
            }
        };

        match poll {
            Poll::Ready(Ok(_)) => {
                parent.record("timed_out", false);
            }
            Poll::Ready(Err(ref e)) => {
                tracing::error!(err = %e, "Future timed out");
                parent.record("timed_out", true);
            }
            _ => {}
        }

        poll
    }
}
