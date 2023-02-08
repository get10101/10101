use lightning::util::logger::Level;
use lightning::util::logger::Logger;
use lightning::util::logger::Record as LnRecord;
use tracing_log::log;
use tracing_log::log::Metadata;

#[derive(Copy, Clone)]
pub(crate) struct TracingLogger;

impl Logger for TracingLogger {
    fn log(&self, record: &LnRecord) {
        let level = match record.level {
            Level::Gossip | Level::Trace => log::Level::Trace,
            Level::Debug => log::Level::Debug,
            Level::Info => log::Level::Info,
            Level::Warn => log::Level::Warn,
            Level::Error => log::Level::Error,
        };

        tracing_log::format_trace(
            &log::Record::builder()
                .level(level)
                .args(record.args)
                .target(record.module_path)
                .module_path_static(Some(record.module_path))
                .file_static(Some(record.file))
                .line(Some(record.line))
                .build(),
        )
        .expect("to be able to format a log record as a trace")
    }
}
