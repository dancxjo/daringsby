use crate::Sensor;
use crate::wits::memory::QdrantClient;
use crate::{ImageData, Sensation};
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::error;

/// Information about a detected face.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FaceInfo {
    /// Cropped face image.
    pub crop: ImageData,
    /// Embedding vector describing the face.
    pub embedding: Vec<f32>,
}

/// Trait for extracting embeddings from images.
#[async_trait]
pub trait FaceDetector: Send + Sync {
    /// Return cropped faces paired with vector embeddings.
    async fn detect_faces(&self, image: &ImageData) -> Result<Vec<(ImageData, Vec<f32>)>>;
}

/// Dummy detector returning the entire image as one face for offline tests.
#[derive(Clone, Default)]
pub struct DummyDetector;

#[async_trait]
impl FaceDetector for DummyDetector {
    async fn detect_faces(&self, image: &ImageData) -> Result<Vec<(ImageData, Vec<f32>)>> {
        Ok(vec![(image.clone(), vec![0.0])])
    }
}

/// Sensor that emits [`FaceInfo`] sensations.
pub struct FaceSensor {
    detector: Arc<dyn FaceDetector>,
    qdrant: QdrantClient,
    tx: mpsc::UnboundedSender<Sensation>,
}

impl FaceSensor {
    /// Create a new sensor using the given `detector`, `qdrant` client and output channel `tx`.
    pub fn new(
        detector: Arc<dyn FaceDetector>,
        qdrant: QdrantClient,
        tx: mpsc::UnboundedSender<Sensation>,
    ) -> Self {
        Self {
            detector,
            qdrant,
            tx,
        }
    }
}

#[async_trait]
impl Sensor<ImageData> for FaceSensor {
    async fn sense(&self, input: ImageData) {
        match self.detector.detect_faces(&input).await {
            Ok(faces) => {
                for (crop, embed) in faces {
                    if let Err(e) = self.qdrant.store_face_vector(&embed).await {
                        error!(?e, "failed storing face vector");
                    }
                    let info = FaceInfo {
                        crop,
                        embedding: embed,
                    };
                    if let Err(e) = self.tx.send(Sensation::Of(Box::new(info))) {
                        error!(?e, "failed sending face info");
                    }
                }
            }
            Err(e) => error!(?e, "face detection failed"),
        }
    }

    fn description(&self) -> String {
        "Face sensor".into()
    }
}
