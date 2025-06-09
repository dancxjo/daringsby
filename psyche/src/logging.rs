use crate::bus::{Event, EventBus};
use log::{Level, LevelFilter, Log, Metadata, Record, SetLoggerError};
use once_cell::sync::OnceCell;
use std::sync::Arc;

struct BusLogger;
static BUS: OnceCell<Arc<EventBus>> = OnceCell::new();

impl Log for BusLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            if let Some(bus) = BUS.get() {
                let msg = format!("{}", record.args());
                bus.send(Event::Log(msg));
            }
        }
    }

    fn flush(&self) {}
}

static LOGGER: BusLogger = BusLogger;

/// Initialize global logging to route messages through the event bus.
pub fn init(bus: Arc<EventBus>) -> Result<(), SetLoggerError> {
    let _ = BUS.set(bus);
    log::set_logger(&LOGGER)?;
    log::set_max_level(LevelFilter::Info);
    Ok(())
}
