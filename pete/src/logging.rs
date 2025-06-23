use std::io::{self, Write};
use tokio::sync::broadcast;
use tracing_subscriber::{EnvFilter, fmt};

/// Initialize logging to stdout and broadcast log lines over the provided channel.
///
/// ```
/// use tokio::sync::broadcast;
/// use pete::init_logging;
///
/// let (tx, _rx) = broadcast::channel(10);
/// init_logging(tx);
/// ```
pub fn init_logging(tx: broadcast::Sender<String>) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("debug"));
    fmt()
        .with_env_filter(filter)
        .with_writer(move || TeeWriter {
            stdout: std::io::stdout(),
            tx: tx.clone(),
        })
        .init();
}

/// Writer that duplicates all output to a broadcast channel.
struct TeeWriter {
    stdout: std::io::Stdout,
    tx: broadcast::Sender<String>,
}

impl Write for TeeWriter {
    /// Writes to stdout and forwards the line to the broadcast channel.
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = self.stdout.write(buf)?;
        if let Ok(s) = std::str::from_utf8(buf) {
            let _ = self.tx.send(s.trim_end().to_string());
        }
        Ok(n)
    }

    /// Flushes the underlying stdout writer.
    fn flush(&mut self) -> io::Result<()> {
        self.stdout.flush()
    }
}
