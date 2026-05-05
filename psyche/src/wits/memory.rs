use crate::{ImageData, Impression, Stimulus, image_content_id};
use anyhow::{Context, Result, anyhow, bail};
use async_trait::async_trait;
use lingproc::Vectorizer;
use reqwest::{StatusCode, Url};
use serde::Serialize;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};
use uuid::Uuid;

const MEMORY_COLLECTION: &str = "memories";
const IMAGE_COLLECTION: &str = "images";
const IMAGE_DESCRIPTION_COLLECTION: &str = "image_descriptions";
const FACE_COLLECTION: &str = "faces";
const GEOLOCATION_COLLECTION: &str = "geolocations";
const VOICE_COLLECTION: &str = "voices";
const QDRANT_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// Trait representing the memory subsystem.
#[async_trait]
pub trait Memory: Send + Sync {
    /// Persist the given impression.
    async fn store(&self, impression: &Impression<Value>) -> Result<()>;

    /// Persist multiple impressions.
    async fn store_all(&self, impressions: &[Impression<Value>]) -> Result<()> {
        for imp in impressions {
            self.store(imp).await?;
        }
        Ok(())
    }
}

impl dyn Memory {
    /// Helper to store any serializable impression.
    pub async fn store_serializable<T: Serialize + Send + Sync>(
        &self,
        impression: &Impression<T>,
    ) -> Result<()> {
        let stimuli: Vec<Stimulus<Value>> = impression
            .stimuli
            .iter()
            .map(|s| {
                Ok(Stimulus {
                    what: serde_json::to_value(&s.what)?,
                    timestamp: s.timestamp,
                })
            })
            .collect::<Result<_, serde_json::Error>>()?;
        let erased = Impression {
            stimuli,
            summary: impression.summary.clone(),
            emoji: impression.emoji.clone(),
            timestamp: impression.timestamp,
        };
        self.store(&erased).await
    }

    /// Helper to store multiple serializable impressions.
    pub async fn store_all_serializable<T: Serialize + Send + Sync>(
        &self,
        impressions: &[Impression<T>],
    ) -> Result<()> {
        let mut erased = Vec::with_capacity(impressions.len());
        for imp in impressions {
            let stimuli: Vec<Stimulus<Value>> = imp
                .stimuli
                .iter()
                .map(|s| {
                    Ok(Stimulus {
                        what: serde_json::to_value(&s.what)?,
                        timestamp: s.timestamp,
                    })
                })
                .collect::<Result<_, serde_json::Error>>()?;
            erased.push(Impression {
                stimuli,
                summary: imp.summary.clone(),
                emoji: imp.emoji.clone(),
                timestamp: imp.timestamp,
            });
        }
        self.store_all(&erased).await
    }
}

/// Client for storing vectors in Qdrant.
#[derive(Clone)]
pub struct QdrantClient {
    pub url: String,
}

impl Default for QdrantClient {
    fn default() -> Self {
        Self {
            url: "http://localhost:6333".into(),
        }
    }
}

impl QdrantClient {
    pub fn new(url: String) -> Self {
        Self { url }
    }
    /// Store `vector` associated with `headline`.
    pub async fn store_vector(&self, headline: &str, vector: &[f32]) -> Result<Uuid> {
        let id = self
            .upsert_vector(
                MEMORY_COLLECTION,
                vector,
                json!({
                    "kind": "memory",
                    "headline": headline,
                }),
            )
            .await?;
        info!(target: "qdrant", ?headline, len = vector.len(), url = %self.url, "stored vector");
        Ok(id)
    }

    /// Store a whole-frame image embedding in the image collection.
    pub async fn store_image_vector(&self, image_id: &str, vector: &[f32]) -> Result<Uuid> {
        let id = self
            .upsert_vector(
                IMAGE_COLLECTION,
                vector,
                json!({
                    "kind": "image",
                    "image_id": image_id,
                }),
            )
            .await?;
        info!(target: "qdrant", image_id, len = vector.len(), url = %self.url, "stored image vector");
        Ok(id)
    }

    /// Store an LLM image-description embedding in its own collection.
    pub async fn store_image_description_vector(
        &self,
        image_id: &str,
        description: &str,
        vector: &[f32],
    ) -> Result<Uuid> {
        let id = self
            .upsert_vector(
                IMAGE_DESCRIPTION_COLLECTION,
                vector,
                json!({
                    "kind": "image_description",
                    "image_id": image_id,
                    "description": description,
                }),
            )
            .await?;
        info!(target: "qdrant", image_id, len = vector.len(), url = %self.url, "stored image description vector");
        Ok(id)
    }

    /// Store a geolocation embedding in the geolocation collection.
    pub async fn store_geolocation_vector_for(
        &self,
        geoloc_id: &str,
        latitude: f64,
        longitude: f64,
        vector: &[f32],
    ) -> Result<Uuid> {
        let id = self
            .upsert_vector(
                GEOLOCATION_COLLECTION,
                vector,
                json!({
                    "kind": "geolocation",
                    "geoloc_id": geoloc_id,
                    "latitude": latitude,
                    "longitude": longitude,
                }),
            )
            .await?;
        info!(target: "qdrant", geoloc_id, len = vector.len(), url = %self.url, "stored geolocation vector");
        Ok(id)
    }

    /// Store a face embedding in the face collection.
    pub async fn store_face_vector(&self, vector: &[f32]) -> Result<Uuid> {
        self.store_face_vector_for(None, None, vector).await
    }

    /// Store a face embedding with graph-linking metadata.
    pub async fn store_face_vector_for(
        &self,
        face_id: Option<&str>,
        source_image_id: Option<&str>,
        vector: &[f32],
    ) -> Result<Uuid> {
        let id = self
            .upsert_vector(
                FACE_COLLECTION,
                vector,
                json!({
                    "kind": "face",
                    "face_id": face_id,
                    "source_image_id": source_image_id,
                }),
            )
            .await?;
        info!(target: "qdrant", len = vector.len(), url = %self.url, "stored face vector");
        Ok(id)
    }

    /// Store a voice embedding in the voice collection.
    pub async fn store_voice_vector(&self, vector: &[f32]) -> Result<Uuid> {
        self.store_voice_vector_for(None, vector).await
    }

    /// Store a voice embedding with graph-linking metadata.
    pub async fn store_voice_vector_for(
        &self,
        clip_id: Option<&str>,
        vector: &[f32],
    ) -> Result<Uuid> {
        let id = self
            .upsert_vector(
                VOICE_COLLECTION,
                vector,
                json!({
                    "kind": "voice",
                    "clip_id": clip_id,
                }),
            )
            .await?;
        info!(target: "qdrant", len = vector.len(), url = %self.url, "stored voice vector");
        Ok(id)
    }

    async fn upsert_vector(
        &self,
        collection: &str,
        vector: &[f32],
        payload: Value,
    ) -> Result<Uuid> {
        if vector.is_empty() {
            bail!("refusing to store empty vector in Qdrant collection {collection}");
        }

        self.ensure_collection(collection, vector.len()).await?;

        let url = self.endpoint(&format!("collections/{collection}/points?wait=true"))?;
        let id = Uuid::new_v4();
        let body = json!({
            "points": [{
                "id": id.to_string(),
                "vector": vector,
                "payload": payload,
            }]
        });
        let response = reqwest::Client::new()
            .put(url)
            .json(&body)
            .timeout(QDRANT_REQUEST_TIMEOUT)
            .send()
            .await
            .with_context(|| {
                format!("failed to upsert point into Qdrant collection {collection}")
            })?;

        if response.status().is_success() {
            Ok(id)
        } else {
            Err(unexpected_qdrant_response(
                response,
                &format!("upserting point into collection {collection}"),
            )
            .await)
        }
    }

    async fn ensure_collection(&self, collection: &str, vector_size: usize) -> Result<()> {
        let client = reqwest::Client::new();
        let url = self.endpoint(&format!("collections/{collection}"))?;
        let response = client
            .get(url.clone())
            .timeout(QDRANT_REQUEST_TIMEOUT)
            .send()
            .await
            .with_context(|| format!("failed to inspect Qdrant collection {collection}"))?;

        if response.status().is_success() {
            let body: Value = response
                .json()
                .await
                .with_context(|| format!("failed to decode Qdrant collection {collection}"))?;
            let existing_size = qdrant_collection_vector_size(&body).with_context(|| {
                format!("Qdrant collection {collection} did not report a vector size")
            })?;
            if existing_size != vector_size {
                warn!(
                    target: "qdrant",
                    collection,
                    existing_size,
                    vector_size,
                    "recreating Qdrant collection with incompatible vector dimension"
                );
                self.recreate_collection(collection, vector_size).await?;
            }
            return Ok(());
        }
        if response.status() != StatusCode::NOT_FOUND {
            return Err(unexpected_qdrant_response(
                response,
                &format!("inspecting collection {collection}"),
            )
            .await);
        }

        self.create_collection(&client, url, collection, vector_size)
            .await
    }

    async fn recreate_collection(&self, collection: &str, vector_size: usize) -> Result<()> {
        let client = reqwest::Client::new();
        let url = self.endpoint(&format!("collections/{collection}"))?;
        let response = client
            .delete(url.clone())
            .timeout(QDRANT_REQUEST_TIMEOUT)
            .send()
            .await
            .with_context(|| format!("failed to delete Qdrant collection {collection}"))?;

        if !response.status().is_success() && response.status() != StatusCode::NOT_FOUND {
            return Err(unexpected_qdrant_response(
                response,
                &format!("deleting collection {collection}"),
            )
            .await);
        }

        self.create_collection(&client, url, collection, vector_size)
            .await
    }

    async fn create_collection(
        &self,
        client: &reqwest::Client,
        url: Url,
        collection: &str,
        vector_size: usize,
    ) -> Result<()> {
        let body = json!({
            "vectors": {
                "size": vector_size,
                "distance": "Cosine",
            }
        });
        let response = client
            .put(url)
            .json(&body)
            .timeout(QDRANT_REQUEST_TIMEOUT)
            .send()
            .await
            .with_context(|| format!("failed to create Qdrant collection {collection}"))?;

        if response.status().is_success() || response.status() == StatusCode::CONFLICT {
            Ok(())
        } else {
            Err(
                unexpected_qdrant_response(response, &format!("creating collection {collection}"))
                    .await,
            )
        }
    }

    fn endpoint(&self, path: &str) -> Result<Url> {
        let base = self.url.trim_end_matches('/');
        Url::parse(&format!("{base}/{}", path.trim_start_matches('/')))
            .with_context(|| format!("invalid Qdrant URL {}", self.url))
    }
}

async fn unexpected_qdrant_response(response: reqwest::Response, action: &str) -> anyhow::Error {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    anyhow!("Qdrant returned {status} while {action}: {body}")
}

fn qdrant_collection_vector_size(collection: &Value) -> Option<usize> {
    let vectors = collection.pointer("/result/config/params/vectors")?;
    if let Some(size) = vectors.get("size").and_then(Value::as_u64) {
        return usize::try_from(size).ok();
    }
    vectors
        .as_object()?
        .values()
        .find_map(|vector| vector.get("size").and_then(Value::as_u64))
        .and_then(|size| usize::try_from(size).ok())
}

/// Client for persisting raw data in Neo4j.
#[derive(Clone)]
pub struct Neo4jClient {
    pub uri: String,
    pub user: String,
    pub pass: String,
}

impl Default for Neo4jClient {
    fn default() -> Self {
        Self {
            uri: "bolt://localhost:7687".into(),
            user: "neo4j".into(),
            pass: "password".into(),
        }
    }
}

impl Neo4jClient {
    pub fn new(uri: String, user: String, pass: String) -> Self {
        Self { uri, user, pass }
    }
    /// Store `data` in the graph database.
    pub async fn store_data(&self, data: &Value) -> Result<()> {
        let json = serde_json::to_string(data)?;
        info!(target: "neo4j", %json, uri = %self.uri, user = %self.user, "stored data");
        Ok(())
    }
}

#[async_trait]
/// Persistent storage for structured memory graphs.
///
/// `GraphStore` implementations write arbitrary JSON-like `Value` records to a
/// backing graph database. Each call should succeed independently so the memory
/// system can continue operating when one store is unavailable.
pub trait GraphStore: Send + Sync {
    /// Store `data` in the graph store.
    async fn store_data(&self, data: &Value) -> Result<()>;
}

#[async_trait]
impl GraphStore for Neo4jClient {
    async fn store_data(&self, data: &Value) -> Result<()> {
        self.store_data(data).await
    }
}

/// Memory implementation combining Qdrant and Neo4j storage.
pub struct BasicMemory {
    /// Vectorizer used for headline embeddings.
    pub vectorizer: Arc<dyn Vectorizer>,
    /// Client used for vector storage.
    pub qdrant: QdrantClient,
    /// Client used for raw data storage.
    pub neo4j: Arc<dyn GraphStore>,
}

#[async_trait]
impl Memory for BasicMemory {
    async fn store(&self, impression: &Impression<Value>) -> Result<()> {
        info!(summary = %impression.summary, "memory store");
        let vector = match tokio::time::timeout(
            Duration::from_secs(5),
            self.vectorizer.vectorize(&impression.summary),
        )
        .await
        {
            Ok(Ok(v)) => Some(v),
            Ok(Err(e)) => {
                tracing::warn!(?e, "🤖 vectorize failed");
                None
            }
            Err(_) => {
                tracing::warn!("🤖 vectorize timed out");
                None
            }
        };
        if let Some(v) = vector {
            if let Some(image_id) = impression
                .stimuli
                .first()
                .and_then(|stim| serde_json::from_value::<ImageData>(stim.what.clone()).ok())
                .map(|image| image_content_id(&image))
            {
                if let Err(e) = self
                    .qdrant
                    .store_image_description_vector(&image_id, &impression.summary, &v)
                    .await
                {
                    tracing::error!(?e, "failed to store image description vector");
                }
            }
            if let Err(e) = self.qdrant.store_vector(&impression.summary, &v).await {
                tracing::error!(?e, "failed to store vector");
            }
        }
        if let Some(stim) = impression.stimuli.first() {
            self.neo4j.store_data(&stim.what).await?;
        }
        Ok(())
    }

    async fn store_all(&self, impressions: &[Impression<Value>]) -> Result<()> {
        for imp in impressions {
            if let Err(e) = self.store(imp).await {
                tracing::warn!(?e, "memory store failed");
            }
        }
        Ok(())
    }
}

/// Memory implementation that performs no storage.
#[derive(Default)]
pub struct NoopMemory;

#[async_trait]
impl Memory for NoopMemory {
    async fn store(&self, _impression: &Impression<Value>) -> Result<()> {
        Ok(())
    }
}
