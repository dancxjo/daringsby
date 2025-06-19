use std::io::{self, Write};
use tokio::sync::broadcast;
use tracing_subscriber::fmt;

/// Initialize logging to stdout and broadcast log lines over the provided channel.
pub fn init_logging(tx: broadcast::Sender<String>) {
    fmt()
        .with_writer(move || TeeWriter {
            stdout: std::io::stdout(),
            tx: tx.clone(),
        })
        .init();
}

struct TeeWriter {
    stdout: std::io::Stdout,
    tx: broadcast::Sender<String>,
}

impl Write for TeeWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = self.stdout.write(buf)?;
        if let Ok(s) = std::str::from_utf8(buf) {
            let _ = self.tx.send(s.trim_end().to_string());
        }
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stdout.flush()
    }
}
