use crate::future_ext::FutureExt;
use futures::future::RemoteHandle;
use futures::Future;
use std::collections::HashMap;
use std::fmt::Display;
use std::hash;
use tracing::Instrument;

/// Struct controlling the lifetime of the async tasks, such as
/// running actors and periodic notifications. If it gets dropped, all
/// tasks are cancelled.
///
/// Each task is associated with a key, so that it can be removed from
/// the internal `HashMap` whenever consumers want to stop tracking
/// the corresponding task. By removing it from the `HashMap` we free
/// up space.
pub struct TaskMap<K>(HashMap<K, RemoteHandle<()>>);

impl<K> TaskMap<K>
where
    K: Eq + hash::Hash,
{
    /// Spawn the task on the runtime and remember the handle.
    ///
    /// The task will be stopped if this instance of [`TaskMap`] goes
    /// out of scope.
    pub fn add(&mut self, key: K, f: impl Future<Output = ()> + Send + 'static) {
        let handle = f.spawn_with_handle();
        self.0.insert(key, handle);
    }

    /// Spawn a fallible task on the runtime and remember the handle.
    ///
    /// The task will be stopped if this instance of [`TaskMap`] goes
    /// out of scope. If the task fails, the `err_handler` will be
    /// invoked.
    pub fn add_fallible<E, EF>(
        &mut self,
        key: K,
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
        self.0.insert(key, handle);
    }

    /// Remove the handle of a task from the internal `HashMap`.
    ///
    /// After removing a task which is present in the `HashMap`, the
    /// `RemoteHandle` will be dropped, effectively ending the task if
    /// it was still ongoing.
    pub fn remove(&mut self, key: &K) {
        self.0.remove(key);
    }
}

impl<K> Default for TaskMap<K> {
    fn default() -> Self {
        Self(HashMap::new())
    }
}
