use async_trait::async_trait;
use glob::glob;
use std::path::PathBuf;
use tokio::{fs, sync::mpsc, time::{self, Duration}};

use crate::{Sensation, Sensor};

/// Reads JPEG files from disk as simulated webcam frames.
pub struct EyeSensor {
    paths: Vec<PathBuf>,
    index: usize,
    interval: Duration,
}

impl EyeSensor {
    /// Create a new sensor that cycles files matching `pattern`.
    pub fn new(pattern: &str, interval: Duration) -> std::io::Result<Self> {
        let paths = glob(pattern)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.msg))?
            .filter_map(Result::ok)
            .collect();
        Ok(Self { paths, index: 0, interval })
    }
}

#[async_trait]
impl Sensor for EyeSensor {
    async fn run(&mut self, tx: mpsc::Sender<Sensation>) {
        let mut ticker = time::interval(self.interval);
        loop {
            ticker.tick().await;
            if self.paths.is_empty() {
                continue;
            }
            if self.index >= self.paths.len() {
                self.index = 0;
            }
            let path = self.paths[self.index].clone();
            self.index += 1;
            match fs::read(&path).await {
                Ok(bytes) => {
                    if tx.send(Sensation::saw(bytes)).await.is_err() {
                        break;
                    }
                }
                Err(e) => log::error!("eye sensor error: {e}"),
            }
        }
    }
}
