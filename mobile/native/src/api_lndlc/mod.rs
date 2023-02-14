use anyhow::Result;
use state::Storage;
use tokio::runtime::Runtime;

const ELECTRS_ORIGIN: &str = "tcp://localhost:50000";

pub mod lndlc;

/// Lazily creates a multi threaded runtime with the the number of worker threads corresponding to
/// the number of available cores.
fn runtime() -> Result<&'static Runtime> {
    static RUNTIME: Storage<Runtime> = Storage::new();

    if RUNTIME.try_get().is_none() {
        let runtime = Runtime::new()?;
        RUNTIME.set(runtime);
    }

    Ok(RUNTIME.get())
}
