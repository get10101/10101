use lightning::util::logger::Logger;
use lightning::util::logger::Record;

#[derive(Copy, Clone)]
pub(crate) struct TracingLogger;

impl Logger for TracingLogger {
    fn log(&self, record: &Record) {
        match record.level {
            lightning::util::logger::Level::Gossip => {
                tracing::trace!(target: "ldk", "{}", record.args.to_string())
            }
            lightning::util::logger::Level::Trace => {
                tracing::trace!(target: "ldk", "{}", record.args.to_string())
            }
            lightning::util::logger::Level::Debug => {
                tracing::debug!(target: "ldk", "{}", record.args.to_string())
            }
            lightning::util::logger::Level::Info => {
                tracing::info!(target: "ldk", "{}", record.args.to_string())
            }
            lightning::util::logger::Level::Warn => {
                tracing::warn!(target: "ldk", "{}", record.args.to_string())
            }
            lightning::util::logger::Level::Error => {
                tracing::error!(target: "ldk", "{}", record.args.to_string())
            }
        };
    }
}
