use log::{LevelFilter, Metadata, Record};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct SimpleLogger {
    buffer: Arc<Mutex<VecDeque<String>>>,
    level: LevelFilter,
}

impl SimpleLogger {
    pub fn init(level: LevelFilter) -> Arc<Self> {
        let logger = Arc::new(Self {
            buffer: Arc::new(Mutex::new(VecDeque::with_capacity(100))),
            level,
        });
        log::set_boxed_logger(Box::new(logger.clone())).expect("set logger");
        log::set_max_level(level);
        logger
    }

    pub fn dump(&self) -> String {
        let buf = self.buffer.lock().unwrap();
        buf.iter().cloned().collect::<Vec<_>>().join("\n")
    }
}

impl log::Log for SimpleLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            println!("[{}] {}", record.level(), record.args());
            let mut buf = self.buffer.lock().unwrap();
            buf.push_back(format!("[{}] {}", record.level(), record.args()));
            if buf.len() > 100 {
                buf.pop_front();
            }
        }
    }

    fn flush(&self) {}
}
