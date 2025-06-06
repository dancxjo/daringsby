use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::Sensation;

/// Trait implemented by all sensors that produce [`Sensation`]s.
#[async_trait]
pub trait Sensor: Send {
    /// Start streaming sensations to the provided channel.
    async fn run(&mut self, tx: mpsc::Sender<Sensation>);
}
