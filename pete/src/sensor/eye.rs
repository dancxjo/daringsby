use async_trait::async_trait;
use chrono::Utc;
use psyche::{ImageData, Sensation, Sensor, image_captured_at};
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, watch};
use tracing::trace;

/// Sensor that forwards webcam images to the psyche.
#[derive(Clone)]
pub struct EyeSensor {
    forward: Option<mpsc::Sender<Sensation>>,
    latest: Option<Arc<Mutex<Option<ImageData>>>>,
    latest_tx: Option<watch::Sender<Option<ImageData>>>,
}

impl EyeSensor {
    /// Create a new `EyeSensor` using the provided channel.
    pub fn new(forward: mpsc::Sender<Sensation>) -> Self {
        Self {
            forward: Some(forward),
            latest: None,
            latest_tx: None,
        }
    }

    /// Create a new `EyeSensor` that also writes the latest image to `latest`.
    pub fn with_latest(
        forward: mpsc::Sender<Sensation>,
        latest: Arc<Mutex<Option<ImageData>>>,
    ) -> Self {
        Self {
            forward: Some(forward),
            latest: Some(latest),
            latest_tx: None,
        }
    }

    /// Create a new `EyeSensor` that publishes the latest image as shared state.
    pub fn latest_only(
        latest: Arc<Mutex<Option<ImageData>>>,
        latest_tx: watch::Sender<Option<ImageData>>,
    ) -> Self {
        Self {
            forward: None,
            latest: Some(latest),
            latest_tx: Some(latest_tx),
        }
    }

    /// Create a new `EyeSensor` that forwards sensations and publishes latest-frame state.
    pub fn with_latest_stream(
        forward: mpsc::Sender<Sensation>,
        latest: Arc<Mutex<Option<ImageData>>>,
        latest_tx: watch::Sender<Option<ImageData>>,
    ) -> Self {
        Self {
            forward: Some(forward),
            latest: Some(latest),
            latest_tx: Some(latest_tx),
        }
    }
}

#[async_trait]
impl Sensor<ImageData> for EyeSensor {
    async fn sense(&self, image: ImageData) {
        trace!("eye sensed image");
        if let Some(buf) = &self.latest {
            *buf.lock().unwrap() = Some(image.clone());
        }
        if let Some(tx) = &self.latest_tx {
            let _ = tx.send(Some(image.clone()));
        }
        if let Some(forward) = &self.forward {
            let occurred_at = image_captured_at(&image).unwrap_or_else(Utc::now);
            let _ = forward.send(Sensation::of_at(image, occurred_at)).await;
        }
    }

    fn describe(&self) -> &'static str {
        "You can see through a webcam. Every few seconds, a new image is \
passed to your perception system. You can describe what you see and recognize people's faces."
    }
}
