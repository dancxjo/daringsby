use async_trait::async_trait;
use psyche::{ImageData, Sensation, Sensor};
use tokio::sync::mpsc;
use tracing::{debug, info};

/// Sensor that forwards webcam images to the psyche.
#[derive(Clone)]
pub struct EyeSensor {
    forward: mpsc::UnboundedSender<Sensation>,
}

impl EyeSensor {
    /// Create a new `EyeSensor` using the provided channel.
    pub fn new(forward: mpsc::UnboundedSender<Sensation>) -> Self {
        Self { forward }
    }
}

#[async_trait]
impl Sensor<ImageData> for EyeSensor {
    async fn sense(&self, image: ImageData) {
        info!("eye sensed image");
        debug!("eye sensed image");
        let _ = self.forward.send(Sensation::Of(Box::new(image)));
    }

    fn describe(&self) -> &'static str {
        "Pete can see through a webcam. Every few seconds, a new image is \
passed to his perception system. He can describe what he sees and recognize people's faces."
    }
}
