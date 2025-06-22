use async_trait::async_trait;
use chrono::Utc;
use psyche::{Heartbeat, Sensation, Sensor};
use rand::Rng;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::info;

/// Sensor emitting a timestamp every ~60 seconds.
#[derive(Clone)]
pub struct HeartbeatSensor;

impl HeartbeatSensor {
    /// Spawn a new heartbeat loop forwarding sensations through `forward`.
    pub fn new(forward: mpsc::Sender<Sensation>) -> Self {
        Self::spawn(forward, Duration::from_secs(55), 10);
        Self
    }

    #[cfg(test)]
    pub fn test_interval(forward: mpsc::Sender<Sensation>, secs: u64) -> Self {
        Self::spawn(forward, Duration::from_secs(secs), 0);
        Self
    }

    fn spawn(forward: mpsc::Sender<Sensation>, base: Duration, range: u64) {
        tokio::spawn(async move {
            loop {
                let secs = rand::thread_rng().gen_range(0..=range);
                let wait = base + Duration::from_secs(secs);
                tokio::time::sleep(wait).await;
                let beat = Heartbeat {
                    timestamp: Utc::now(),
                };
                info!("heartbeat");
                let _ = forward.send(Sensation::Of(Box::new(beat))).await;
            }
        });
    }
}

#[async_trait]
impl Sensor<()> for HeartbeatSensor {
    async fn sense(&self, _input: ()) {}

    fn describe(&self) -> &'static str {
        "Heartbeat: Announces the time periodically."
    }
}
