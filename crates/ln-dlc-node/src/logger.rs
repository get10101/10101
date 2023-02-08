use lightning::util::logger::Logger;
use lightning::util::logger::Record;

#[derive(Copy, Clone)]
pub(crate) struct TracingLogger;

impl Logger for TracingLogger {
    fn log(&self, record: &Record) {
        match record.level {
            lightning::util::logger::Level::Gossip => {
                println!("GOSSIP: {}", record.args.to_string());
            }
            lightning::util::logger::Level::Trace => {
                println!("TRACE: {}", record.args.to_string());
            }
            lightning::util::logger::Level::Debug => {
                println!("DEBUG: {}", record.args.to_string());
            }
            lightning::util::logger::Level::Info => {
                println!("INFO: {}", record.args.to_string());
            }
            lightning::util::logger::Level::Warn => {
                println!("WARN: {}", record.args.to_string());
            }
            lightning::util::logger::Level::Error => {
                println!("ERROR: {}", record.args.to_string());
            }
        }
    }
}
