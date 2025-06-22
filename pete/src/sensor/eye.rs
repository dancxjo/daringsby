use async_trait::async_trait;
use psyche::{ImageData, Sensation, Sensor};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tracing::{debug, info};

/// Sensor that forwards webcam images to the psyche.
#[derive(Clone)]
pub struct EyeSensor {
    forward: mpsc::UnboundedSender<Sensation>,
    latest: Option<Arc<Mutex<Option<ImageData>>>>,
}

impl EyeSensor {
    /// Create a new `EyeSensor` using the provided channel.
    pub fn new(forward: mpsc::UnboundedSender<Sensation>) -> Self {
        Self {
            forward,
            latest: None,
        }
    }

    /// Create a new `EyeSensor` that also writes the latest image to `latest`.
    pub fn with_latest(
        forward: mpsc::UnboundedSender<Sensation>,
        latest: Arc<Mutex<Option<ImageData>>>,
    ) -> Self {
        Self {
            forward,
            latest: Some(latest),
        }
    }
}

#[async_trait]
impl Sensor<ImageData> for EyeSensor {
    async fn sense(&self, image: ImageData) {
        info!("eye sensed image");
        debug!("eye sensed image");
        if let Some(buf) = &self.latest {
            *buf.lock().unwrap() = Some(image.clone());
        }
        let _ = self.forward.send(Sensation::Of(Box::new(image)));
    }

    fn describe(&self) -> &'static str {
        "Pete can see through a webcam. Every few seconds, a new image is \
passed to his perception system. He can describe what he sees and recognize people's faces."
    }
}
