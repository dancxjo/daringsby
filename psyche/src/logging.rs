use crate::bus::{Event, global_bus};
use log::{Level, LevelFilter, Log, Metadata, Record, SetLoggerError};

struct BusLogger;

impl Log for BusLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let msg = format!("{}", record.args());
            global_bus().send(Event::Log(msg));
        }
    }

    fn flush(&self) {}
}

static LOGGER: BusLogger = BusLogger;

/// Initialize global logging to route messages through the event bus.
pub fn init() -> Result<(), SetLoggerError> {
    log::set_logger(&LOGGER)?;
    log::set_max_level(LevelFilter::Info);
    Ok(())
}
