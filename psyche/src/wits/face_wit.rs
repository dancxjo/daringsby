use crate::traits::wit::Wit;
use crate::wits::memory::QdrantClient;
use crate::{ImageData, Impression, Stimulus};
use anyhow::Result;
use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use image::load_from_memory;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use tracing::debug;

/// Trait for extracting face embeddings from an image.
#[async_trait]
pub trait FaceDetector: Send + Sync {
    /// Return cropped faces paired with vector embeddings.
    async fn detect_faces(&self, image: &ImageData) -> Result<Vec<(ImageData, Vec<f32>)>>;
}

/// Dummy detector returning the entire image as one face.
#[derive(Clone, Default)]
pub struct DummyDetector;

#[async_trait]
impl FaceDetector for DummyDetector {
    async fn detect_faces(&self, image: &ImageData) -> Result<Vec<(ImageData, Vec<f32>)>> {
        if let Ok(bytes) = BASE64.decode(&image.base64) {
            let _ = load_from_memory(&bytes);
        }
        Ok(vec![(image.clone(), vec![0.0])])
    }
}

/// Wit detecting faces and storing their embeddings.
pub struct FaceWit {
    detector: Arc<dyn FaceDetector>,
    qdrant: QdrantClient,
    buffer: Mutex<Vec<ImageData>>,
    tx: Option<broadcast::Sender<crate::WitReport>>,
}

impl FaceWit {
    /// Debug label for this Wit.
    pub const LABEL: &'static str = "Face";

    /// Create a new `FaceWit` using the given detector and Qdrant client.
    pub fn new(detector: Arc<dyn FaceDetector>, qdrant: QdrantClient) -> Self {
        Self {
            detector,
            qdrant,
            buffer: Mutex::new(Vec::new()),
            tx: None,
        }
    }

    /// Create a `FaceWit` emitting [`WitReport`]s using `tx`.
    pub fn with_debug(
        detector: Arc<dyn FaceDetector>,
        qdrant: QdrantClient,
        tx: broadcast::Sender<crate::WitReport>,
    ) -> Self {
        Self {
            detector,
            qdrant,
            buffer: Mutex::new(Vec::new()),
            tx: Some(tx),
        }
    }
}

#[async_trait]
impl crate::traits::wit::Wit<ImageData, ImageData> for FaceWit {
    async fn observe(&self, input: ImageData) {
        self.buffer.lock().unwrap().push(input);
    }

    async fn tick(&self) -> Vec<Impression<ImageData>> {
        let img = {
            let mut buf = self.buffer.lock().unwrap();
            if buf.is_empty() {
                return Vec::new();
            }
            buf.remove(0)
        };
        debug!("face wit processing image");
        let faces = match self.detector.detect_faces(&img).await {
            Ok(f) => f,
            Err(_) => return Vec::new(),
        };
        let mut out = Vec::new();
        for (face_img, vec) in faces {
            let _ = self.qdrant.store_face_vector(&vec).await;
            if let Some(tx) = &self.tx {
                if crate::debug::debug_enabled(Self::LABEL).await {
                    let _ = tx.send(crate::WitReport {
                        name: Self::LABEL.into(),
                        prompt: "face detected".into(),
                        output: "stored".into(),
                    });
                }
            }
            out.push(Impression::new(
                vec![Stimulus::new(face_img)],
                "I'm seeing a face.".to_string(),
                None::<String>,
            ));
        }
        out
    }

    fn debug_label(&self) -> &'static str {
        Self::LABEL
    }
}

#[async_trait]
impl crate::traits::observer::SensationObserver for FaceWit {
    async fn observe_sensation(&self, s: &crate::Sensation) {
        if let crate::Sensation::Of(any) = s {
            if let Some(img) = any.downcast_ref::<ImageData>() {
                self.observe(img.clone()).await;
            }
        }
    }
}
