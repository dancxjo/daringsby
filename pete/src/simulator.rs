use std::sync::Arc;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use psyche::{Ear, ImageData, Sensor};
use tracing::info;

/// Utility for feeding fake sensations to a [`Psyche`].
#[derive(Clone)]
pub struct Simulator {
    ear: Arc<dyn Ear>,
    eye: Arc<dyn Sensor<ImageData>>,
}

impl Simulator {
    /// Create a new `Simulator` using the provided ear and eye.
    pub fn new(ear: Arc<dyn Ear>, eye: Arc<dyn Sensor<ImageData>>) -> Self {
        Self { ear, eye }
    }

    /// Send a text message as if spoken by the user.
    pub async fn text(&self, msg: &str) {
        info!(%msg, "simulator text");
        self.ear.hear_user_say(msg).await;
    }

    /// Send raw image bytes with MIME type to the psyche.
    pub async fn image(&self, mime: &str, bytes: &[u8]) {
        info!(%mime, size = bytes.len(), "simulator image");
        let data = BASE64.encode(bytes);
        let img = ImageData {
            mime: mime.to_string(),
            base64: data,
        };
        self.eye.sense(img).await;
    }
}
