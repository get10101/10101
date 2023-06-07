use crate::future_ext::FutureExt;
use futures::future::RemoteHandle;
use futures::Future;
use std::fmt::Display;
use tracing::Instrument;

#[cfg(feature = "xtra")]
pub use actor_scoped::*;
pub use task_map::*;

#[cfg(feature = "xtra")]
mod actor_scoped;
mod task_map;

/// Struct controlling the lifetime of the async tasks, such as
/// running actors and periodic notifications. If it gets dropped, all
/// tasks are cancelled.
#[derive(Default)]
pub struct Tasks(Vec<RemoteHandle<()>>);

impl Tasks {
    /// Spawn the task on the runtime and remember the handle.
    ///
    /// The task will be stopped if this instance of [`Tasks`] goes
    /// out of scope.
    pub fn add(&mut self, f: impl Future<Output = ()> + Send + 'static) {
        let handle = f.spawn_with_handle();
        self.0.push(handle);
    }

    /// Spawn a fallible task on the runtime and remember the handle.
    ///
    /// The task will be stopped if this instance of [`Tasks`] goes
    /// out of scope. If the task fails, the `err_handler` will be
    /// invoked.
    pub fn add_fallible<E, EF>(
        &mut self,
        f: impl Future<Output = Result<(), E>> + Send + 'static,
        err_handler: impl FnOnce(E) -> EF + Send + 'static,
    ) where
        E: Display + Send + 'static,
        EF: Future<Output = ()> + Send + 'static,
    {
        let fut = async move {
            match f.await {
                Ok(()) => {}
                Err(err) => {
                    let span = tracing::error_span!("fallible task handle_error", %err);
                    err_handler(err).instrument(span).await
                }
            }
        };

        let handle = fut.spawn_with_handle();
        self.0.push(handle);
    }
}

#[cfg(feature = "xtra")]
impl xtra::spawn::Spawner for Tasks {
    fn spawn<F: Future<Output = ()> + Send + 'static>(&mut self, fut: F) {
        self.add(fut);
    }
}
