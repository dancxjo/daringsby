use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::Sensation;

#[async_trait]
pub trait Sensor: Send {
    async fn run(&mut self, tx: mpsc::Sender<Sensation>);
}
