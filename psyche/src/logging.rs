use crate::bus::{Event, EventBus};
use log::{Level, LevelFilter, Log, Metadata, Record, SetLoggerError};
use once_cell::sync::OnceCell;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::{Arc, Mutex};

struct BusLogger;
static BUS: OnceCell<Arc<EventBus>> = OnceCell::new();
static LOGFILE: OnceCell<Mutex<File>> = OnceCell::new();

impl Log for BusLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            if let Some(bus) = BUS.get() {
                let msg = format!("{}", record.args());
                bus.send(Event::Log(msg.clone()));
                println!("{}", msg);
                if let Some(file) = LOGFILE.get() {
                    let mut file = file.lock().unwrap();
                    let _ = writeln!(file, "{}", msg);
                }
            }
        }
    }

    fn flush(&self) {}
}

static LOGGER: BusLogger = BusLogger;

/// Initialize global logging to route messages through the event bus.
pub fn init(bus: Arc<EventBus>) -> Result<(), SetLoggerError> {
    let _ = BUS.set(bus);
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("debug.log")
        .unwrap();
    let _ = LOGFILE.set(Mutex::new(file));
    log::set_logger(&LOGGER)?;
    log::set_max_level(LevelFilter::Info);
    Ok(())
}
