use async_trait::async_trait;
use chrono::Utc;
use tokio::{
    sync::mpsc,
    time::{self, Duration},
};

use crate::{Sensation, Sensor};

/// Periodically emits a "heartbeat" [`Sensation`].
pub struct HeartbeatSensor {
    interval: Duration,
}

impl HeartbeatSensor {
    /// Create a new sensor that ticks at the given interval.
    pub fn new(interval: Duration) -> Self {
        Self { interval }
    }
}

#[async_trait]
impl Sensor for HeartbeatSensor {
    /// Continuously send heartbeat messages until the channel closes.
    async fn run(&mut self, tx: mpsc::Sender<Sensation>) {
        let mut ticker = time::interval(self.interval);
        loop {
            ticker.tick().await;
            let when = Utc::now();
            let msg = format!("Heartbeat at {} - I'm alive", when.to_rfc3339());
            if tx.send(Sensation::new(msg, None::<String>)).await.is_err() {
                break;
            }
        }
    }
}
