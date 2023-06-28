use lightning::util::logger::Level;
use lightning::util::logger::Logger;
use lightning::util::logger::Record as LnRecord;

#[derive(Clone)]
pub struct TracingLogger {
    pub alias: String,
}

impl Logger for TracingLogger {
    fn log(&self, record: &LnRecord) {
        let level = match record.level {
            Level::Gossip | Level::Trace => log::Level::Trace,
            Level::Debug => log::Level::Debug,
            Level::Info => log::Level::Info,
            Level::Warn => log::Level::Warn,
            Level::Error => log::Level::Error,
        };

        #[cfg(test)]
        let target = {
            // We must add the alias to the _end_ of the target because otherwise our `EnvFilter`
            // configuration will not work
            format!("{}[{}]", record.module_path, self.alias)
        };
        #[cfg(not(test))]
        let target = record.module_path.to_string();

        tracing_log::format_trace(
            &log::Record::builder()
                .level(level)
                .args(record.args)
                .target(&target)
                .module_path(Some(&target))
                .file_static(Some(record.file))
                .line(Some(record.line))
                .build(),
        )
        .expect("to be able to format a log record as a trace")
    }
}
