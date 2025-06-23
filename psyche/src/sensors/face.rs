use crate::topics::TopicBus;
use crate::traits::Sensor;
use crate::wits::memory::QdrantClient;
use crate::{ImageData, Sensation};
use anyhow::Result;
use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use tracing::{debug, error};

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

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    dot / (norm_a * norm_b + 1e-5)
}

/// Sensor that emits [`FaceInfo`] sensations.
pub struct FaceSensor {
    detector: Arc<dyn FaceDetector>,
    qdrant: QdrantClient,
    bus: TopicBus,
    last_face: Mutex<Option<Vec<f32>>>,
}

impl FaceSensor {
    /// Create a new sensor using the given `detector`, `qdrant` client and output channel `tx`.
    pub fn new(detector: Arc<dyn FaceDetector>, qdrant: QdrantClient, bus: TopicBus) -> Self {
        Self {
            detector,
            qdrant,
            bus,
            last_face: Mutex::new(None),
        }
    }
}

#[async_trait]
impl Sensor<ImageData> for FaceSensor {
    async fn sense(&self, input: ImageData) {
        match self.detector.detect_faces(&input).await {
            Ok(faces) => {
                for (crop, embed) in faces {
                    let skip = {
                        let mut last = self.last_face.lock().unwrap();
                        let similar = last
                            .as_ref()
                            .map_or(false, |p| cosine_similarity(p, &embed) > 0.95);
                        if !similar {
                            *last = Some(embed.clone());
                        }
                        similar
                    };
                    if skip {
                        debug!("skipping similar face detection");
                        continue;
                    }
                    if let Err(e) = self.qdrant.store_face_vector(&embed).await {
                        error!(?e, "failed storing face vector");
                    }
                    let info = FaceInfo {
                        crop,
                        embedding: embed,
                    };
                    self.bus.publish(
                        crate::topics::Topic::Sensation,
                        Sensation::Of(Box::new(info)),
                    );
                }
            }
            Err(e) => error!(?e, "face detection failed"),
        }
    }

    fn describe(&self) -> &'static str {
        "Pete tries to recognize faces in the images he sees. If he sees the \
same face often, he may remember it."
    }
}
