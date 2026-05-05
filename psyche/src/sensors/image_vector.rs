use crate::topics::TopicBus;
use crate::traits::Sensor;
use crate::wits::memory::QdrantClient;
use crate::{ImageData, ImageEmbedding, Sensation, image_captured_at, image_content_id};
use anyhow::{Context, Result};
use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use image::imageops::FilterType;
use lingproc::math::cosine_similarity;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use tracing::{debug, error};

/// Whole-frame image embedding extractor.
#[async_trait]
pub trait WholeImageVectorizer: Send + Sync {
    async fn vectorize_image(&self, image: &ImageData) -> Result<Vec<f32>>;

    fn model_name(&self) -> &'static str;
}

/// Whole-frame embedder backed by `ruvector-cnn`.
pub struct RuVectorCnnImageVectorizer {
    embedder: Mutex<ruvector_cnn::CnnEmbedder>,
}

impl RuVectorCnnImageVectorizer {
    pub fn new() -> Result<Self> {
        Ok(Self {
            embedder: Mutex::new(ruvector_cnn::CnnEmbedder::new(
                ruvector_cnn::EmbeddingConfig::default(),
            )?),
        })
    }
}

impl Default for RuVectorCnnImageVectorizer {
    fn default() -> Self {
        Self::new().expect("default ruvector-cnn image vectorizer should initialize")
    }
}

#[async_trait]
impl WholeImageVectorizer for RuVectorCnnImageVectorizer {
    async fn vectorize_image(&self, image: &ImageData) -> Result<Vec<f32>> {
        let image = image.clone();
        let input_size = self.embedder.lock().unwrap().input_size();
        let rgba = tokio::task::spawn_blocking(move || decode_rgba(&image, input_size))
            .await
            .context("image vectorization task failed")??;
        let embedder = self
            .embedder
            .lock()
            .map_err(|_| anyhow::anyhow!("image vectorizer lock poisoned"))?;
        embedder
            .extract(&rgba, input_size, input_size)
            .context("ruvector-cnn extraction failed")
    }

    fn model_name(&self) -> &'static str {
        "ruvector-cnn/default"
    }
}

fn decode_rgba(image: &ImageData, input_size: u32) -> Result<Vec<u8>> {
    let bytes = BASE64_STANDARD
        .decode(image.base64.trim().as_bytes())
        .context("failed to decode image payload")?;
    let img = image::load_from_memory(&bytes)
        .context("failed to decode image")?
        .resize_exact(input_size, input_size, FilterType::Triangle)
        .to_rgba8();
    Ok(img.into_raw())
}

/// Sensor that emits and stores one whole-image vector for each distinct frame.
pub struct ImageVectorSensor {
    vectorizer: Arc<dyn WholeImageVectorizer>,
    qdrant: QdrantClient,
    bus: TopicBus,
    seen_images: Mutex<HashSet<String>>,
    last_embedding: Mutex<Option<Vec<f32>>>,
}

impl ImageVectorSensor {
    pub fn new(
        vectorizer: Arc<dyn WholeImageVectorizer>,
        qdrant: QdrantClient,
        bus: TopicBus,
    ) -> Self {
        Self {
            vectorizer,
            qdrant,
            bus,
            seen_images: Mutex::new(HashSet::new()),
            last_embedding: Mutex::new(None),
        }
    }
}

#[async_trait]
impl Sensor<ImageData> for ImageVectorSensor {
    async fn sense(&self, input: ImageData) {
        if input.base64.trim().is_empty() {
            return;
        }
        let occurred_at = image_captured_at(&input).unwrap_or_else(chrono::Utc::now);
        let image_id = image_content_id(&input);
        {
            let mut seen = self.seen_images.lock().unwrap();
            if !seen.insert(image_id.clone()) {
                debug!(%image_id, "skipping duplicate image vectorization");
                return;
            }
        }

        let embedding = match self.vectorizer.vectorize_image(&input).await {
            Ok(embedding) => embedding,
            Err(e) => {
                error!(?e, "image vectorization failed");
                return;
            }
        };
        let skip = {
            let mut last = self.last_embedding.lock().unwrap();
            let similar = last
                .as_ref()
                .is_some_and(|prev| cosine_similarity(prev, &embedding) > 0.999);
            if !similar {
                *last = Some(embedding.clone());
            }
            similar
        };
        if skip {
            debug!(%image_id, "skipping nearly identical image embedding");
            return;
        }

        let vector_id = match self.qdrant.store_image_vector(&image_id, &embedding).await {
            Ok(id) => Some(id.to_string()),
            Err(e) => {
                error!(?e, "failed storing image vector");
                None
            }
        };
        self.bus.publish(
            crate::topics::Topic::Sensation,
            Sensation::of_at(
                ImageEmbedding {
                    image: input,
                    image_id,
                    embedding,
                    vector_id,
                    model: Some(self.vectorizer.model_name().to_string()),
                },
                occurred_at,
            ),
        );
    }

    fn describe(&self) -> &'static str {
        "You form a whole-image visual embedding for each distinct camera frame."
    }
}
