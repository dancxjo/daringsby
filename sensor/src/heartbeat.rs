use async_trait::async_trait;
use tokio::{sync::mpsc, time::{self, Duration}};
use chrono::Utc;

use crate::{Sensation, Sensor};

pub struct HeartbeatSensor {
    interval: Duration,
}

impl HeartbeatSensor {
    pub fn new(interval: Duration) -> Self {
        Self { interval }
    }
}

#[async_trait]
impl Sensor for HeartbeatSensor {
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
