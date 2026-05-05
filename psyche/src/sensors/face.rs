use crate::topics::TopicBus;
use crate::traits::Sensor;
use crate::wits::memory::QdrantClient;
use crate::{ImageData, Sensation, image_captured_at, image_content_id};
use anyhow::{Context, Result};
use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use lingproc::math::cosine_similarity;
use std::io::Cursor;
use std::sync::{Arc, Mutex};
use tracing::{debug, error};

/// Information about a detected face.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FaceInfo {
    /// Cropped face image.
    pub crop: ImageData,
    /// Stable content id of the cropped face image.
    pub face_id: String,
    /// Stable content id of the source image.
    pub source_image_id: String,
    /// Embedding vector describing the face.
    pub embedding: Vec<f32>,
    /// Qdrant vector id for the face embedding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vector_id: Option<String>,
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

/// Detector backed by the `face_id` ONNX pipeline.
pub struct FaceIdDetector {
    analyzer: Arc<Mutex<face_id::analyzer::FaceAnalyzer>>,
}

impl FaceIdDetector {
    /// Create a detector using the default Hugging Face models.
    pub async fn from_hf() -> Result<Self> {
        let analyzer = face_id::analyzer::FaceAnalyzer::from_hf()
            .build()
            .await
            .context("failed to initialize face_id analyzer")?;
        Ok(Self {
            analyzer: Arc::new(Mutex::new(analyzer)),
        })
    }
}

#[async_trait]
impl FaceDetector for FaceIdDetector {
    async fn detect_faces(&self, image: &ImageData) -> Result<Vec<(ImageData, Vec<f32>)>> {
        let image = image.clone();
        let analyzer = Arc::clone(&self.analyzer);
        tokio::task::spawn_blocking(move || {
            if image.base64.trim().is_empty() {
                return Ok(Vec::new());
            }

            let bytes = BASE64_STANDARD
                .decode(image.base64.trim().as_bytes())
                .context("failed to decode image payload")?;
            let img = image::load_from_memory(&bytes).context("failed to decode image")?;
            let faces = analyzer
                .lock()
                .map_err(|_| anyhow::anyhow!("face analyzer lock poisoned"))?
                .analyze(&img)
                .context("face_id analysis failed")?;

            faces
                .into_iter()
                .map(|face| {
                    let crop = crop_face(&img, &face.detection)?;
                    Ok((crop, face.embedding))
                })
                .collect()
        })
        .await
        .context("face detection task failed")?
    }
}

fn crop_face(
    img: &image::DynamicImage,
    detection: &face_id::detector::DetectedFace,
) -> Result<ImageData> {
    let width = img.width();
    let height = img.height();
    let bbox = detection.to_absolute(width, height).bbox;

    let x1 = bbox.x1.floor().clamp(0.0, width.saturating_sub(1) as f32) as u32;
    let y1 = bbox.y1.floor().clamp(0.0, height.saturating_sub(1) as f32) as u32;
    let x2 = bbox.x2.ceil().clamp(1.0, width as f32) as u32;
    let y2 = bbox.y2.ceil().clamp(1.0, height as f32) as u32;

    let crop = if x2 > x1 && y2 > y1 {
        img.crop_imm(x1, y1, x2 - x1, y2 - y1)
    } else {
        img.clone()
    };

    let mut bytes = Cursor::new(Vec::new());
    crop.write_to(&mut bytes, image::ImageFormat::Jpeg)
        .context("failed to encode face crop")?;

    Ok(ImageData {
        mime: "image/jpeg".to_string(),
        base64: BASE64_STANDARD.encode(bytes.into_inner()),
        captured_at: None,
    })
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
        let occurred_at = image_captured_at(&input).unwrap_or_else(chrono::Utc::now);
        match self.detector.detect_faces(&input).await {
            Ok(faces) => {
                debug!(count = faces.len(), "face detector completed");
                for (mut crop, embed) in faces {
                    if crop.captured_at.is_none() {
                        crop.captured_at = Some(occurred_at.to_rfc3339());
                    }
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
                    let source_image_id = image_content_id(&input);
                    let face_id = image_content_id(&crop);
                    let vector_id = match self
                        .qdrant
                        .store_face_vector_for(Some(&face_id), Some(&source_image_id), &embed)
                        .await
                    {
                        Ok(id) => Some(id.to_string()),
                        Err(e) => {
                            error!(?e, "failed storing face vector");
                            None
                        }
                    };
                    let info = FaceInfo {
                        crop,
                        face_id,
                        source_image_id,
                        embedding: embed,
                        vector_id,
                    };
                    self.bus.publish(
                        crate::topics::Topic::Sensation,
                        Sensation::of_at(info, occurred_at),
                    );
                }
            }
            Err(e) => error!(?e, "face detection failed"),
        }
    }

    fn describe(&self) -> &'static str {
        "You try to recognize faces in the images you see. If you see the \
same face often, you may remember it."
    }
}
