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

    fn description(&self) -> String {
        "Webcam: Streams images from your environment.".to_string()
    }
}
