use crate::{
    AudioClip, GeoLoc, Heartbeat, ImageData, Impression, ObjectInfo, Stimulus, audio_clip_id,
    geoloc_content_id, image_content_id,
};
use anyhow::{Context, Result, anyhow, bail};
use async_trait::async_trait;
use lingproc::Vectorizer;
use lingproc::math::cosine_similarity;
use reqwest::{StatusCode, Url};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;
use tracing::{info, warn};
use uuid::Uuid;

const MEMORY_COLLECTION: &str = "memories";
const IMAGE_COLLECTION: &str = "images";
const IMAGE_DESCRIPTION_COLLECTION: &str = "image_descriptions";
const SCENE_VECTOR_COLLECTION: &str = "scene_vectors";
const FACE_COLLECTION: &str = "faces";
const GEOLOCATION_COLLECTION: &str = "geolocations";
const VOICE_COLLECTION: &str = "voices";
const QDRANT_VECTOR_COLLECTIONS: &[&str] = &[
    MEMORY_COLLECTION,
    IMAGE_COLLECTION,
    IMAGE_DESCRIPTION_COLLECTION,
    SCENE_VECTOR_COLLECTION,
    FACE_COLLECTION,
    GEOLOCATION_COLLECTION,
    VOICE_COLLECTION,
];
const QDRANT_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const NEO4J_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// Return the Qdrant collections Pete writes vector embeddings into.
pub fn qdrant_vector_collections() -> &'static [&'static str] {
    QDRANT_VECTOR_COLLECTIONS
}

/// One vector point loaded from Qdrant for offline analysis.
#[derive(Clone, Debug, PartialEq)]
pub struct QdrantVectorPoint {
    /// Qdrant point id.
    pub point_id: String,
    /// Dense vector payload.
    pub vector: Vec<f32>,
    /// Qdrant payload metadata.
    pub payload: Value,
}

/// One member of a discovered vector cluster.
#[derive(Clone, Debug, PartialEq)]
pub struct VectorClusterMember {
    /// Qdrant point id.
    pub point_id: String,
    /// Average cosine similarity from this member to the other members.
    pub average_similarity: f32,
}

/// A connected component of nearby vectors in one Qdrant collection.
#[derive(Clone, Debug, PartialEq)]
pub struct VectorCluster {
    /// Stable graph id for the cluster.
    pub cluster_id: String,
    /// Qdrant collection analyzed.
    pub collection: String,
    /// Minimum pairwise edge similarity used to form the component.
    pub threshold: f32,
    /// Mean vector of the cluster members.
    pub centroid: Vec<f32>,
    /// Mean pairwise cosine similarity across all members.
    pub mean_similarity: f32,
    /// Cluster members, sorted by point id for stable output.
    pub members: Vec<VectorClusterMember>,
}

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
        self.store_vector_for_node(headline, None, vector).await
    }

    /// Store a memory vector with an explicit Neo4j node back-reference.
    pub async fn store_vector_for_node(
        &self,
        headline: &str,
        neo4j_node_id: Option<&str>,
        vector: &[f32],
    ) -> Result<Uuid> {
        let id = self
            .upsert_vector(
                MEMORY_COLLECTION,
                vector,
                json!({
                    "kind": "memory",
                    "headline": headline,
                    "neo4j_node_id": neo4j_node_id,
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
                    "neo4j_node_id": image_id,
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
        self.store_image_description_vector_for_node(image_id, description, image_id, &[], vector)
            .await
    }

    /// Store an image-description embedding with Neo4j graph back-references.
    pub async fn store_image_description_vector_for_node(
        &self,
        image_id: &str,
        description: &str,
        neo4j_node_id: &str,
        related_neo4j_node_ids: &[&str],
        vector: &[f32],
    ) -> Result<Uuid> {
        self.store_image_description_vector_for_node_with_model(
            image_id,
            description,
            neo4j_node_id,
            related_neo4j_node_ids,
            None,
            vector,
        )
        .await
    }

    /// Store an image-description embedding with graph back-references and model metadata.
    pub async fn store_image_description_vector_for_node_with_model(
        &self,
        image_id: &str,
        description: &str,
        neo4j_node_id: &str,
        related_neo4j_node_ids: &[&str],
        model: Option<&str>,
        vector: &[f32],
    ) -> Result<Uuid> {
        let id = self
            .upsert_vector(
                IMAGE_DESCRIPTION_COLLECTION,
                vector,
                json!({
                    "kind": "image_description",
                    "image_id": image_id,
                    "neo4j_node_id": neo4j_node_id,
                    "related_neo4j_node_ids": related_neo4j_node_ids,
                    "model": model,
                    "description": description,
                }),
            )
            .await?;
        info!(target: "qdrant", image_id, len = vector.len(), url = %self.url, "stored image description vector");
        Ok(id)
    }

    /// Store a CLIP scene embedding in its own collection.
    pub async fn store_scene_vector_for_sensation(
        &self,
        image_id: &str,
        sensation_id: Option<&str>,
        model: &str,
        vector: &[f32],
    ) -> Result<Uuid> {
        let id = self
            .upsert_vector(
                SCENE_VECTOR_COLLECTION,
                vector,
                json!({
                    "kind": "scene",
                    "image_id": image_id,
                    "neo4j_node_id": image_id,
                    "source_image_id": image_id,
                    "sensation_id": sensation_id,
                    "model": model,
                }),
            )
            .await?;
        info!(target: "qdrant", image_id, len = vector.len(), url = %self.url, "stored scene vector");
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
                    "neo4j_node_id": geoloc_id,
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
        self.store_face_vector_for_sensation(face_id, source_image_id, None, vector)
            .await
    }

    /// Store a face embedding with graph and source sensation metadata.
    pub async fn store_face_vector_for_sensation(
        &self,
        face_id: Option<&str>,
        source_image_id: Option<&str>,
        sensation_id: Option<&str>,
        vector: &[f32],
    ) -> Result<Uuid> {
        let id = self
            .upsert_vector(
                FACE_COLLECTION,
                vector,
                json!({
                    "kind": "face",
                    "face_id": face_id,
                    "neo4j_node_id": face_id,
                    "source_image_id": source_image_id,
                    "sensation_id": sensation_id,
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
        self.store_voice_vector_for_sensation(clip_id, None, None, vector)
            .await
    }

    /// Store a voice embedding with graph and source sensation metadata.
    pub async fn store_voice_vector_for_sensation(
        &self,
        clip_id: Option<&str>,
        sensation_id: Option<&str>,
        user_id: Option<&str>,
        vector: &[f32],
    ) -> Result<Uuid> {
        let id = self
            .upsert_vector(
                VOICE_COLLECTION,
                vector,
                json!({
                    "kind": "voice",
                    "clip_id": clip_id,
                    "neo4j_node_id": clip_id,
                    "sensation_id": sensation_id,
                    "user_id": user_id,
                }),
            )
            .await?;
        info!(target: "qdrant", len = vector.len(), url = %self.url, "stored voice vector");
        Ok(id)
    }

    /// Load vector points from a Qdrant collection using the scroll API.
    pub async fn scroll_vectors(
        &self,
        collection: &str,
        max_points: usize,
        page_size: usize,
    ) -> Result<Vec<QdrantVectorPoint>> {
        self.scroll_vectors_with_missing_policy(collection, max_points, page_size, false)
            .await
            .map(|points| points.unwrap_or_default())
    }

    /// Load vector points when a collection exists, returning `None` for absent collections.
    pub async fn scroll_vectors_if_collection_exists(
        &self,
        collection: &str,
        max_points: usize,
        page_size: usize,
    ) -> Result<Option<Vec<QdrantVectorPoint>>> {
        self.scroll_vectors_with_missing_policy(collection, max_points, page_size, true)
            .await
    }

    async fn scroll_vectors_with_missing_policy(
        &self,
        collection: &str,
        max_points: usize,
        page_size: usize,
        allow_missing_collection: bool,
    ) -> Result<Option<Vec<QdrantVectorPoint>>> {
        if max_points == 0 {
            return Ok(Some(Vec::new()));
        }
        let page_size = page_size.max(1).min(max_points);
        let client = reqwest::Client::new();
        let url = self.endpoint(&format!("collections/{collection}/points/scroll"))?;
        let mut points = Vec::new();
        let mut offset: Option<Value> = None;

        while points.len() < max_points {
            let limit = (max_points - points.len()).min(page_size);
            let mut body = json!({
                "limit": limit,
                "with_payload": true,
                "with_vector": true,
            });
            if let Some(offset) = &offset {
                body["offset"] = offset.clone();
            }
            let response = client
                .post(url.clone())
                .json(&body)
                .timeout(QDRANT_REQUEST_TIMEOUT)
                .send()
                .await
                .with_context(|| format!("failed to scroll Qdrant collection {collection}"))?;
            if !response.status().is_success() {
                if allow_missing_collection && response.status() == StatusCode::NOT_FOUND {
                    return Ok(None);
                }
                return Err(unexpected_qdrant_response(
                    response,
                    &format!("scrolling collection {collection}"),
                )
                .await);
            }
            let body: Value = response.json().await.with_context(|| {
                format!("failed to decode Qdrant scroll response for {collection}")
            })?;
            let page = qdrant_scroll_points(&body)
                .with_context(|| format!("Qdrant scroll response for {collection} was invalid"))?;
            let next_offset = body.pointer("/result/next_page_offset").cloned();
            if page.is_empty() {
                break;
            }
            points.extend(page);
            offset = next_offset.filter(|value| !value.is_null());
            if offset.is_none() {
                break;
            }
        }

        Ok(Some(points))
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

fn qdrant_scroll_points(response: &Value) -> Result<Vec<QdrantVectorPoint>> {
    response
        .pointer("/result/points")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(qdrant_vector_point)
        .collect()
}

fn qdrant_vector_point(value: Value) -> Result<QdrantVectorPoint> {
    let object = value
        .as_object()
        .context("Qdrant scroll point was not an object")?;
    let point_id = qdrant_point_id(
        object
            .get("id")
            .context("Qdrant scroll point is missing id")?,
    )?;
    let vector = qdrant_point_vector(
        object
            .get("vector")
            .context("Qdrant scroll point is missing vector")?,
    )?;
    Ok(QdrantVectorPoint {
        point_id,
        vector,
        payload: object.get("payload").cloned().unwrap_or_else(|| json!({})),
    })
}

fn qdrant_point_id(value: &Value) -> Result<String> {
    match value {
        Value::String(id) => Ok(id.clone()),
        Value::Number(number) => Ok(number.to_string()),
        _ => bail!("Qdrant point id was not a string or number"),
    }
}

fn qdrant_point_vector(value: &Value) -> Result<Vec<f32>> {
    if let Some(vector) = qdrant_vector_array(value) {
        return vector;
    }
    let object = value
        .as_object()
        .context("Qdrant point vector was not an array or named vector object")?;
    object
        .values()
        .find_map(qdrant_vector_array)
        .context("Qdrant named vector object did not contain a dense vector")?
}

fn qdrant_vector_array(value: &Value) -> Option<Result<Vec<f32>>> {
    let values = value.as_array()?;
    Some(
        values
            .iter()
            .map(|value| {
                value
                    .as_f64()
                    .map(|value| value as f32)
                    .context("Qdrant dense vector contained a non-number")
            })
            .collect(),
    )
}

/// Find connected components of vectors whose cosine similarity meets `threshold`.
pub fn find_vector_clusters(
    collection: &str,
    points: &[QdrantVectorPoint],
    threshold: f32,
    min_size: usize,
) -> Vec<VectorCluster> {
    let min_size = min_size.max(2);
    if points.len() < min_size {
        return Vec::new();
    }

    let mut components = DisjointSet::new(points.len());
    for left in 0..points.len() {
        for right in (left + 1)..points.len() {
            if points[left].vector.len() != points[right].vector.len() {
                continue;
            }
            if cosine_similarity(&points[left].vector, &points[right].vector) >= threshold {
                components.union(left, right);
            }
        }
    }

    let mut grouped: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for index in 0..points.len() {
        let root = components.find(index);
        grouped.entry(root).or_default().push(index);
    }

    let mut clusters = grouped
        .values()
        .filter(|indices| indices.len() >= min_size)
        .map(|indices| vector_cluster(collection, points, indices, threshold))
        .collect::<Vec<_>>();
    clusters.sort_by(|left, right| {
        right
            .members
            .len()
            .cmp(&left.members.len())
            .then_with(|| left.cluster_id.cmp(&right.cluster_id))
    });
    clusters
}

fn vector_cluster(
    collection: &str,
    points: &[QdrantVectorPoint],
    indices: &[usize],
    threshold: f32,
) -> VectorCluster {
    let dimension = indices
        .iter()
        .find_map(|index| points.get(*index))
        .map_or(0, |point| point.vector.len());
    let mut centroid = vec![0.0; dimension];
    for index in indices {
        if let Some(point) = points.get(*index) {
            for (target, value) in centroid.iter_mut().zip(&point.vector) {
                *target += value;
            }
        }
    }
    for value in &mut centroid {
        *value /= indices.len() as f32;
    }

    let mut total_similarity = 0.0;
    let mut pair_count = 0usize;
    let members = indices
        .iter()
        .map(|index| {
            let point = &points[*index];
            let mut member_similarity = 0.0;
            let mut member_pairs = 0usize;
            for other in indices {
                if other == index {
                    continue;
                }
                let similarity = cosine_similarity(&point.vector, &points[*other].vector);
                member_similarity += similarity;
                member_pairs += 1;
                if *index < *other {
                    total_similarity += similarity;
                    pair_count += 1;
                }
            }
            VectorClusterMember {
                point_id: point.point_id.clone(),
                average_similarity: if member_pairs == 0 {
                    1.0
                } else {
                    member_similarity / member_pairs as f32
                },
            }
        })
        .collect::<Vec<_>>();

    let mut sorted_members = members;
    sorted_members.sort_by(|left, right| left.point_id.cmp(&right.point_id));
    let member_ids = sorted_members
        .iter()
        .map(|member| member.point_id.as_str())
        .collect::<Vec<_>>()
        .join(",");
    VectorCluster {
        cluster_id: stable_bytes_id(
            "cluster",
            format!("{collection}:{threshold:.4}:{member_ids}").as_bytes(),
        ),
        collection: collection.to_string(),
        threshold,
        centroid,
        mean_similarity: if pair_count == 0 {
            1.0
        } else {
            total_similarity / pair_count as f32
        },
        members: sorted_members,
    }
}

struct DisjointSet {
    parents: Vec<usize>,
    ranks: Vec<u8>,
}

impl DisjointSet {
    fn new(len: usize) -> Self {
        Self {
            parents: (0..len).collect(),
            ranks: vec![0; len],
        }
    }

    fn find(&mut self, index: usize) -> usize {
        if self.parents[index] != index {
            self.parents[index] = self.find(self.parents[index]);
        }
        self.parents[index]
    }

    fn union(&mut self, left: usize, right: usize) {
        let left_root = self.find(left);
        let right_root = self.find(right);
        if left_root == right_root {
            return;
        }
        match self.ranks[left_root].cmp(&self.ranks[right_root]) {
            std::cmp::Ordering::Less => self.parents[left_root] = right_root,
            std::cmp::Ordering::Greater => self.parents[right_root] = left_root,
            std::cmp::Ordering::Equal => {
                self.parents[right_root] = left_root;
                self.ranks[left_root] += 1;
            }
        }
    }
}

/// Client for persisting raw data in Neo4j.
#[derive(Clone)]
pub struct Neo4jClient {
    pub uri: String,
    pub user: String,
    pub pass: String,
    constraint_ensured: Arc<AtomicBool>,
}

impl Default for Neo4jClient {
    fn default() -> Self {
        Self {
            uri: "bolt://localhost:7687".into(),
            user: "neo4j".into(),
            pass: "password".into(),
            constraint_ensured: Arc::new(AtomicBool::new(false)),
        }
    }
}

/// Audio clip loaded directly from the graph store.
#[derive(Clone, Debug)]
pub struct GraphAudioClip {
    /// Stable graph node id for the `AudioClip`.
    pub id: String,
    /// Audio payload and metadata stored on the graph node.
    pub clip: AudioClip,
    /// Graph observation timestamp, when present.
    pub occurred_at: Option<String>,
    /// Source `Sensation` node that observed this clip, when present.
    pub sensation_id: Option<String>,
}

/// Ordered audio clips selected for aggregate transcription.
#[derive(Clone, Debug)]
pub struct GraphAudioClipWindow {
    /// The newest clip in the window that had not yet received a big transcription.
    pub anchor_id: String,
    /// Source clips in playback order.
    pub clips: Vec<GraphAudioClip>,
}

/// Source audio clip participating in a big transcription consolidation.
#[derive(Clone, Debug)]
pub struct GraphConsolidatedSpeechSource {
    /// Source order within the aggregate transcription.
    pub index: usize,
    /// Source clip and graph metadata.
    pub clip: GraphAudioClip,
    /// Start offset from the beginning of the aggregate audio.
    pub start_ms: u32,
    /// End offset from the beginning of the aggregate audio.
    pub end_ms: u32,
    /// First-order transcription nodes attached directly to this clip.
    pub transcription_ids: Vec<String>,
}

/// Big transcription and source clips ready to be collapsed into one audio node.
#[derive(Clone, Debug)]
pub struct GraphConsolidatedSpeechCandidate {
    /// Existing big transcription node id.
    pub transcription_id: String,
    /// Big transcript text.
    pub transcript: String,
    /// Aggregate source start timestamp, when known.
    pub source_started_at: Option<String>,
    /// Aggregate source end timestamp, when known.
    pub source_ended_at: Option<String>,
    /// Source clips in playback order.
    pub sources: Vec<GraphConsolidatedSpeechSource>,
}

/// Result of consolidating a big transcription into one audio clip.
#[derive(Clone, Debug, PartialEq)]
pub struct GraphSpeechConsolidationReport {
    /// Existing big transcription that was consolidated.
    pub transcription_id: String,
    /// New fused `AudioClip` node id.
    pub consolidated_audio_clip_id: String,
    /// Original clips represented by the fused audio.
    pub source_audio_clip_ids: Vec<String>,
    /// First-order transcription ids removed during cleanup.
    pub deleted_transcription_ids: Vec<String>,
}

/// Audio clip loaded directly from the graph store for offline voice recognition.
#[derive(Clone, Debug)]
pub struct GraphVoiceClip {
    /// Stable graph node id for the `AudioClip`.
    pub id: String,
    /// Audio payload and metadata stored on the graph node.
    pub clip: AudioClip,
    /// Graph observation timestamp, when present.
    pub occurred_at: Option<String>,
    /// Source `Sensation` node that observed this clip, when present.
    pub sensation_id: Option<String>,
}

/// Image frame loaded directly from the graph store for offline processing.
#[derive(Clone, Debug)]
pub struct GraphImageFrame {
    /// Stable graph node id for the `Image`.
    pub id: String,
    /// Image payload and metadata stored on the graph node.
    pub image: ImageData,
    /// Graph observation timestamp, when present.
    pub occurred_at: Option<String>,
    /// Source `Sensation` node that observed this frame, when present.
    pub sensation_id: Option<String>,
}

/// LLM image description ready to be linked into the graph.
#[derive(Clone, Debug)]
pub struct GraphImageDescription {
    /// Stable graph node id for the description.
    pub description_id: String,
    /// Single-sentence description of the source image.
    pub text: String,
    /// Qdrant point id for the text embedding.
    pub vector_id: String,
    /// Text embedding dimension.
    pub embedding_len: usize,
}

/// Geolocation loaded directly from the graph store for offline vectorization.
#[derive(Clone, Debug)]
pub struct GraphGeolocation {
    /// Stable graph node id for the `Geolocation`.
    pub id: String,
    /// Latitude/longitude payload stored on the graph node.
    pub loc: GeoLoc,
    /// Graph observation timestamp, when present.
    pub occurred_at: Option<String>,
    /// Source `Sensation` node that observed this location, when present.
    pub sensation_id: Option<String>,
}

/// Face recognition result ready to be linked into the graph.
#[derive(Clone, Debug)]
pub struct GraphFaceDetection {
    /// Zero-based detection order within the source frame.
    pub index: usize,
    /// Stable graph node id for the cropped face image.
    pub face_id: String,
    /// Cropped face image and capture metadata.
    pub crop: ImageData,
    /// Qdrant point id for the face embedding.
    pub vector_id: String,
    /// Face embedding dimension.
    pub embedding_len: usize,
}

/// Scene-level image vector ready to be linked into the graph.
#[derive(Clone, Debug)]
pub struct GraphSceneVectorization {
    /// Qdrant point id for the scene embedding.
    pub vector_id: String,
    /// Scene embedding dimension.
    pub embedding_len: usize,
}

/// Voice signature extracted from an audio clip.
#[derive(Clone, Debug)]
pub struct GraphVoiceSignature {
    /// Stable speaker/signature id.
    pub user_id: String,
    pub fundamental_frequency: f32,
    pub frequency_range: (f32, f32),
    pub formant_frequencies: Vec<f32>,
    pub speech_rate: f32,
    pub mfcc_signature: Vec<f32>,
    pub spectral_centroid: f32,
    pub jitter: f32,
    pub shimmer: f32,
    pub harmonic_to_noise_ratio: f32,
    pub sample_count: usize,
    pub last_updated: chrono::DateTime<chrono::Utc>,
    pub tags: Vec<String>,
}

/// Voice sample extracted from one source audio clip.
#[derive(Clone, Debug)]
pub struct GraphVoiceSample {
    pub id: String,
    pub user_id: String,
    pub duration_ms: u32,
    pub sample_rate: u32,
    pub fundamental_frequency: f32,
    pub formant_frequencies: Vec<f32>,
    pub mfcc: Vec<f32>,
    pub quality_score: f32,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Voice recognition result ready to be linked into the graph.
#[derive(Clone, Debug)]
pub struct GraphVoiceRecognition {
    pub signature: GraphVoiceSignature,
    pub sample: GraphVoiceSample,
    /// Qdrant point id for the voice embedding.
    pub vector_id: String,
    /// Voice embedding dimension.
    pub embedding_len: usize,
}

/// Speech segment produced by transcribing an `AudioClip`.
#[derive(Clone, Debug)]
pub struct GraphSpeechSegment {
    /// Zero-based segment order in the transcription.
    pub index: usize,
    /// Segment text.
    pub text: String,
    /// Start offset from the beginning of the audio clip.
    pub start_ms: u32,
    /// End offset from the beginning of the audio clip.
    pub end_ms: u32,
    /// Absolute segment start timestamp, when the source clip was timestamped.
    pub occurred_at: Option<String>,
    /// Absolute segment end timestamp, when the source clip was timestamped.
    pub ended_at: Option<String>,
}

/// Audio source data and clip-local offsets for a `SpeechSegment`.
#[derive(Clone, Debug, PartialEq)]
pub struct GraphSpeechSegmentAudio {
    /// Stable graph node id for the speech segment.
    pub segment_id: String,
    /// Segment text.
    pub text: String,
    /// Stable graph node id for the source `AudioClip`.
    pub audio_clip_id: String,
    /// Source audio MIME type.
    pub mime: String,
    /// Source audio payload.
    pub base64: String,
    /// Source sample rate.
    pub sample_rate: u32,
    /// Source channel count.
    pub channels: u16,
    /// Segment start offset from the beginning of the source audio clip.
    pub start_ms: u32,
    /// Segment end offset from the beginning of the source audio clip.
    pub end_ms: u32,
}

/// Source clip span within an aggregate audio transcription.
#[derive(Clone, Debug)]
pub struct GraphAudioSourceSpan {
    /// Zero-based source order within the aggregate audio.
    pub index: usize,
    /// Stable graph node id for the source `AudioClip`.
    pub audio_clip_id: String,
    /// Start offset from the beginning of the aggregate audio.
    pub start_ms: u32,
    /// End offset from the beginning of the aggregate audio.
    pub end_ms: u32,
    /// Absolute source start timestamp, when known.
    pub occurred_at: Option<String>,
    /// Absolute source end timestamp, when known.
    pub ended_at: Option<String>,
    /// Whether this clip was the unprocessed anchor that caused the window to run.
    pub anchor: bool,
    /// Source `Sensation` node that observed this clip, when present.
    pub sensation_id: Option<String>,
}

/// Graph node returned for browser-side visualization.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct GraphNodeSnapshot {
    /// Stable graph node id.
    pub id: String,
    /// Neo4j labels attached to the node.
    pub labels: Vec<String>,
    /// Display-safe node properties. Large payload fields are omitted.
    pub properties: Value,
}

/// Graph relationship returned for browser-side visualization.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct GraphRelationshipSnapshot {
    /// Neo4j relationship element id.
    pub id: String,
    /// Source graph node id.
    pub source: String,
    /// Target graph node id.
    pub target: String,
    /// Neo4j relationship type.
    #[serde(rename = "type")]
    pub relationship_type: String,
    /// Display-safe relationship properties.
    pub properties: Value,
}

/// Full graph node details returned for an inspector view.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct GraphNodeDetails {
    /// Stable graph node id.
    pub id: String,
    /// Neo4j labels attached to the node.
    pub labels: Vec<String>,
    /// Inspector properties. Media payloads are retained; large embeddings are omitted.
    pub properties: Value,
    /// Relationships touching this node.
    pub relationships: Vec<GraphRelationshipSnapshot>,
}

/// Latest graph state for a real-time graph browser.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct GraphSnapshot {
    /// Nodes included in the current graph window.
    pub nodes: Vec<GraphNodeSnapshot>,
    /// Relationships between included nodes.
    pub relationships: Vec<GraphRelationshipSnapshot>,
}

/// One displayable graph event used as input to offline combobulation.
#[derive(Clone, Debug, PartialEq)]
pub struct GraphTimelineItem {
    /// Stable graph node id for the origin event.
    pub id: String,
    /// Stable grouping key for the real-world event this graph node represents.
    pub event_id: String,
    /// Neo4j labels attached to the origin event.
    pub labels: Vec<String>,
    /// Human-readable event text for an LLM timeline.
    pub text: String,
    /// Best-known event timestamp.
    pub occurred_at: String,
}

/// Ordered timeline window selected for offline combobulation.
#[derive(Clone, Debug, PartialEq)]
pub struct GraphTimelineWindow {
    /// The newest source event that had not yet been included in a combobulation run.
    pub anchor_id: String,
    /// Anchor timestamp.
    pub anchor_at: String,
    /// Source events in chronological order.
    pub items: Vec<GraphTimelineItem>,
}

/// LLM awareness summary ready to be linked into the graph.
#[derive(Clone, Debug, PartialEq)]
pub struct GraphAwareness {
    /// Stable graph node id for the awareness statement.
    pub awareness_id: String,
    /// Natural-language awareness summary.
    pub text: String,
    /// Qdrant point id for the summary embedding.
    pub vector_id: String,
    /// Text embedding dimension.
    pub embedding_len: usize,
}

/// Human-readable graph item related to one vector-cluster member.
#[derive(Clone, Debug, PartialEq)]
pub struct GraphClusterItem {
    /// Qdrant vector node id in Neo4j.
    pub vector_id: String,
    /// Source graph node id that owns or explains the vector.
    pub node_id: String,
    /// Source graph labels.
    pub labels: Vec<String>,
    /// Human-readable source text for theme extraction.
    pub text: String,
    /// Human-readable stimuli interpreted by the source node, when present.
    pub stimuli: Vec<String>,
    /// Human-readable graph relationships near the source node.
    pub edges: Vec<String>,
    /// Human-readable neighboring graph nodes near the source node.
    pub neighbors: Vec<String>,
}

/// LLM-generated theme for one vector cluster.
#[derive(Clone, Debug, PartialEq)]
pub struct GraphClusterTheme {
    /// Stable graph node id for the theme.
    pub theme_id: String,
    /// Natural-language description of the common theme.
    pub text: String,
}

impl Neo4jClient {
    pub fn new(uri: String, user: String, pass: String) -> Self {
        Self {
            uri,
            user,
            pass,
            constraint_ensured: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Return the latest `AudioClip` graph node that has no transcript property.
    pub async fn latest_untranscribed_audio_clip(&self) -> Result<Option<GraphAudioClip>> {
        let endpoint = self.http_endpoint()?;
        let rows = query_neo4j_rows(
            &reqwest::Client::new(),
            &endpoint,
            &self.user,
            &self.pass,
            CypherStatement {
                statement: r#"
                    MATCH (a:GraphNode:AudioClip)
                    WHERE a.base64 IS NOT NULL
                      AND a.transcript IS NULL
                      AND NOT (a)-[:HAS_TRANSCRIPTION]->(:GraphNode:Transcription)
                    OPTIONAL MATCH (s:GraphNode:Sensation)-[:OBSERVED]->(a)
                    WITH a, s, coalesce(a.captured_at, a.occurred_at, s.occurred_at, "") AS observed_at
                    RETURN a.id, a.mime, a.base64, a.sample_rate, a.channels, a.captured_at, a.occurred_at, s.id
                    ORDER BY observed_at DESC
                    LIMIT 1
                "#
                .into(),
                parameters: json!({}),
            },
            "finding latest untranscribed audio clip",
        )
        .await?;
        rows.first().map(graph_audio_clip_from_row).transpose()
    }

    /// Return recent `AudioClip` nodes for aggregate transcription.
    ///
    /// The anchor is the latest clip that has not been linked to a big
    /// transcription. The returned window includes that anchor plus earlier
    /// clips, regardless of whether those clips already have first-order
    /// transcripts.
    pub async fn latest_audio_clip_window_for_big_transcription(
        &self,
        limit: usize,
    ) -> Result<Option<GraphAudioClipWindow>> {
        let limit = limit.max(1);
        let endpoint = self.http_endpoint()?;
        let rows = query_neo4j_rows(
            &reqwest::Client::new(),
            &endpoint,
            &self.user,
            &self.pass,
            CypherStatement {
                statement: r#"
                    MATCH (anchor:GraphNode:AudioClip)
                    WHERE anchor.base64 IS NOT NULL
                      AND NOT (anchor)-[:HAS_BIG_TRANSCRIPTION]->(:GraphNode:Transcription)
                    WITH anchor, coalesce(anchor.captured_at, anchor.occurred_at, "") AS anchor_observed_at
                    ORDER BY anchor_observed_at DESC
                    LIMIT 1
                    MATCH (a:GraphNode:AudioClip)
                    WHERE a.base64 IS NOT NULL
                    OPTIONAL MATCH (s:GraphNode:Sensation)-[:OBSERVED]->(a)
                    WITH anchor, anchor_observed_at, a, s, coalesce(a.captured_at, a.occurred_at, s.occurred_at, "") AS observed_at
                    WHERE observed_at <= anchor_observed_at
                    WITH anchor, a, s, observed_at
                    ORDER BY observed_at DESC
                    LIMIT $limit
                    WITH anchor, collect({
                        id: a.id,
                        mime: a.mime,
                        base64: a.base64,
                        sample_rate: a.sample_rate,
                        channels: a.channels,
                        captured_at: a.captured_at,
                        occurred_at: a.occurred_at,
                        sensation_id: s.id,
                        observed_at: observed_at
                    }) AS clips
                    UNWIND reverse(clips) AS clip
                    RETURN anchor.id, clip.id, clip.mime, clip.base64, clip.sample_rate, clip.channels, clip.captured_at, clip.occurred_at, clip.sensation_id
                "#
                .into(),
                parameters: json!({
                    "limit": i64::try_from(limit).unwrap_or(i64::MAX),
                }),
            },
            "finding latest audio clip window for big transcription",
        )
        .await?;
        graph_audio_clip_window_from_rows(&rows)
    }

    /// Return the latest big transcription whose source audio has not been consolidated.
    pub async fn latest_big_transcription_for_speech_consolidation(
        &self,
        min_source_count: usize,
    ) -> Result<Option<GraphConsolidatedSpeechCandidate>> {
        let endpoint = self.http_endpoint()?;
        let rows = query_neo4j_rows(
            &reqwest::Client::new(),
            &endpoint,
            &self.user,
            &self.pass,
            CypherStatement {
                statement: r#"
                    MATCH (t:GraphNode:Transcription)
                    WHERE coalesce(t.kind, "") = "big"
                      AND NOT (t)-[:HAS_CONSOLIDATED_AUDIO]->(:GraphNode:AudioClip)
                      AND toInteger(coalesce(t.source_count, 0)) >= $min_source_count
                    MATCH (a:GraphNode:AudioClip)-[source:HAS_BIG_TRANSCRIPTION]->(t)
                    WHERE a.base64 IS NOT NULL
                    OPTIONAL MATCH (s:GraphNode:Sensation)-[:OBSERVED]->(a)
                    OPTIONAL MATCH (a)-[:HAS_TRANSCRIPTION]->(old:GraphNode:Transcription)
                    WITH t, a, source, s, collect(DISTINCT old.id) AS old_transcription_ids,
                        coalesce(t.transcribed_at, t.source_ended_at, t.source_started_at, "") AS candidate_at
                    ORDER BY toInteger(coalesce(source.source_index, 0)) ASC, a.id
                    WITH t, candidate_at, collect({
                        index: toInteger(coalesce(source.source_index, 0)),
                        id: a.id,
                        mime: a.mime,
                        base64: a.base64,
                        sample_rate: a.sample_rate,
                        channels: a.channels,
                        transcript: a.transcript,
                        captured_at: a.captured_at,
                        occurred_at: a.occurred_at,
                        sensation_id: s.id,
                        start_ms: toInteger(coalesce(source.start_ms, 0)),
                        end_ms: toInteger(coalesce(source.end_ms, 0)),
                        transcription_ids: old_transcription_ids
                    }) AS sources
                    WHERE size(sources) >= $min_source_count
                    RETURN
                        t.id,
                        coalesce(t.text, t.transcript, ""),
                        t.source_started_at,
                        t.source_ended_at,
                        sources
                    ORDER BY candidate_at DESC, t.id
                    LIMIT 1
                "#
                .into(),
                parameters: json!({
                    "min_source_count": i64::try_from(min_source_count.max(1)).unwrap_or(i64::MAX),
                }),
            },
            "finding latest big transcription for speech consolidation",
        )
        .await?;
        rows.first()
            .map(graph_consolidated_speech_candidate_from_row)
            .transpose()
    }

    /// Return the latest `AudioClip` graph node that has no voice-recognition run.
    pub async fn latest_unprocessed_audio_clip_for_voice_recognition(
        &self,
    ) -> Result<Option<GraphVoiceClip>> {
        let endpoint = self.http_endpoint()?;
        let rows = query_neo4j_rows(
            &reqwest::Client::new(),
            &endpoint,
            &self.user,
            &self.pass,
            CypherStatement {
                statement: r#"
                    MATCH (a:GraphNode:AudioClip)
                    WHERE a.base64 IS NOT NULL
                      AND NOT (a)-[:HAS_VOICE_RECOGNITION_RUN]->(:GraphNode:VoiceRecognitionRun)
                    OPTIONAL MATCH (s:GraphNode:Sensation)-[:OBSERVED]->(a)
                    WITH a, s, coalesce(a.captured_at, a.occurred_at, s.occurred_at, "") AS observed_at
                    RETURN a.id, a.mime, a.base64, a.sample_rate, a.channels, a.captured_at, a.occurred_at, s.id
                    ORDER BY observed_at DESC
                    LIMIT 1
                "#
                .into(),
                parameters: json!({}),
            },
            "finding latest unprocessed audio clip for voice recognition",
        )
        .await?;
        rows.first().map(graph_voice_clip_from_row).transpose()
    }

    /// Return the latest `Image` graph node that has no face-recognition run.
    pub async fn latest_unprocessed_image_frame_for_face_recognition(
        &self,
    ) -> Result<Option<GraphImageFrame>> {
        let endpoint = self.http_endpoint()?;
        let rows = query_neo4j_rows(
            &reqwest::Client::new(),
            &endpoint,
            &self.user,
            &self.pass,
            CypherStatement {
                statement: r#"
                    MATCH (i:GraphNode:Image)
                    WHERE i.base64 IS NOT NULL
                      AND NOT (i)-[:HAS_FACE_RECOGNITION_RUN]->(:GraphNode:FaceRecognitionRun)
                    OPTIONAL MATCH (s:GraphNode:Sensation)-[:OBSERVED]->(i)
                    WITH i, s, coalesce(i.captured_at, i.occurred_at, s.occurred_at, "") AS observed_at
                    RETURN i.id, i.mime, i.base64, i.captured_at, i.occurred_at, s.id
                    ORDER BY observed_at DESC
                    LIMIT 1
                "#
                .into(),
                parameters: json!({}),
            },
            "finding latest unprocessed image frame for face recognition",
        )
        .await?;
        rows.first().map(graph_image_frame_from_row).transpose()
    }

    /// Return the latest `Image` graph node that has no scene-vectorization run.
    pub async fn latest_unprocessed_image_frame_for_scene_vectorization(
        &self,
    ) -> Result<Option<GraphImageFrame>> {
        let endpoint = self.http_endpoint()?;
        let rows = query_neo4j_rows(
            &reqwest::Client::new(),
            &endpoint,
            &self.user,
            &self.pass,
            CypherStatement {
                statement: r#"
                    MATCH (i:GraphNode:Image)
                    WHERE i.base64 IS NOT NULL
                      AND NOT (i)-[:HAS_SCENE_VECTORIZATION_RUN]->(:GraphNode:SceneVectorizationRun)
                    OPTIONAL MATCH (s:GraphNode:Sensation)-[:OBSERVED]->(i)
                    WITH i, s, coalesce(i.captured_at, i.occurred_at, s.occurred_at, "") AS observed_at
                    RETURN i.id, i.mime, i.base64, i.captured_at, i.occurred_at, s.id
                    ORDER BY observed_at DESC
                    LIMIT 1
                "#
                .into(),
                parameters: json!({}),
            },
            "finding latest unprocessed image frame for scene vectorization",
        )
        .await?;
        rows.first().map(graph_image_frame_from_row).transpose()
    }

    /// Return the latest `Image` graph node that has no image-description run.
    pub async fn latest_unprocessed_image_frame_for_description(
        &self,
    ) -> Result<Option<GraphImageFrame>> {
        let endpoint = self.http_endpoint()?;
        let rows = query_neo4j_rows(
            &reqwest::Client::new(),
            &endpoint,
            &self.user,
            &self.pass,
            CypherStatement {
                statement: r#"
                    MATCH (i:GraphNode:Image)
                    WHERE i.base64 IS NOT NULL
                      AND NOT (i)-[:HAS_IMAGE_DESCRIPTION_RUN]->(:GraphNode:ImageDescriptionRun)
                    OPTIONAL MATCH (s:GraphNode:Sensation)-[:OBSERVED]->(i)
                    WITH i, s, coalesce(i.captured_at, i.occurred_at, s.occurred_at, "") AS observed_at
                    RETURN i.id, i.mime, i.base64, i.captured_at, i.occurred_at, s.id
                    ORDER BY observed_at DESC
                    LIMIT 1
                "#
                .into(),
                parameters: json!({}),
            },
            "finding latest unprocessed image frame for description",
        )
        .await?;
        rows.first().map(graph_image_frame_from_row).transpose()
    }

    /// Return the latest `Geolocation` graph node that has no geolocation vector.
    pub async fn latest_unprocessed_geolocation_for_vectorization(
        &self,
    ) -> Result<Option<GraphGeolocation>> {
        let endpoint = self.http_endpoint()?;
        let rows = query_neo4j_rows(
            &reqwest::Client::new(),
            &endpoint,
            &self.user,
            &self.pass,
            CypherStatement {
                statement: r#"
                    MATCH (g:GraphNode:Geolocation)
                    WHERE g.latitude IS NOT NULL
                      AND g.longitude IS NOT NULL
                      AND NOT (g)-[:HAS_GEOLOCATION_VECTOR]->(:GraphNode:Vector)
                      AND NOT (g)-[:HAS_GEOLOCATION_VECTORIZATION_RUN]->(:GraphNode:GeolocationVectorizationRun)
                    OPTIONAL MATCH (s:GraphNode:Sensation)-[:OBSERVED]->(g)
                    WITH g, s, coalesce(g.observed_at, g.occurred_at, s.occurred_at, "") AS observed_at
                    RETURN g.id, g.latitude, g.longitude, g.observed_at, g.occurred_at, s.id
                    ORDER BY observed_at DESC
                    LIMIT 1
                "#
                .into(),
                parameters: json!({}),
            },
            "finding latest unprocessed geolocation for vectorization",
        )
        .await?;
        rows.first().map(graph_geolocation_from_row).transpose()
    }

    /// Return a display-oriented snapshot of the latest graph nodes and their relationships.
    pub async fn graph_snapshot(&self, limit: usize) -> Result<GraphSnapshot> {
        let endpoint = self.http_endpoint()?;
        let rows = query_neo4j_rows(
            &reqwest::Client::new(),
            &endpoint,
            &self.user,
            &self.pass,
            CypherStatement {
                statement: r#"
                    MATCH (n:GraphNode)
                    WHERE EXISTS { MATCH (n)--(:GraphNode) }
                    WITH n
                    ORDER BY coalesce(
                        n.occurred_at,
                        n.observed_at,
                        n.captured_at,
                        n.transcribed_at,
                        n.timestamp,
                        ""
                    ) DESC, n.id
                    LIMIT $limit
                    WITH collect(n) AS anchors
                    UNWIND anchors AS anchor
                    MATCH (anchor)--(neighbor:GraphNode)
                    WITH anchors, collect(DISTINCT neighbor) AS neighbors
                    WITH anchors + [neighbor IN neighbors WHERE NOT neighbor IN anchors] AS nodes
                    UNWIND nodes AS n
                    OPTIONAL MATCH (n)-[r]-(m:GraphNode)
                    WHERE m IN nodes
                    WITH nodes, collect(DISTINCT r) AS relationships
                    RETURN
                        [node IN nodes | {
                            id: node.id,
                            labels: labels(node),
                            properties: properties(node)
                        }],
                        [rel IN relationships WHERE rel IS NOT NULL | {
                            id: elementId(rel),
                            source: startNode(rel).id,
                            target: endNode(rel).id,
                            type: type(rel),
                            properties: properties(rel)
                        }]
                "#
                .into(),
                parameters: json!({
                    "limit": i64::try_from(limit).unwrap_or(i64::MAX),
                }),
            },
            "loading graph snapshot",
        )
        .await?;
        rows.first()
            .map(graph_snapshot_from_row)
            .transpose()
            .map(|snapshot| snapshot.unwrap_or_default())
    }

    /// Return full details for a single graph node.
    pub async fn graph_node_details(&self, id: &str) -> Result<Option<GraphNodeDetails>> {
        let endpoint = self.http_endpoint()?;
        let rows = query_neo4j_rows(
            &reqwest::Client::new(),
            &endpoint,
            &self.user,
            &self.pass,
            CypherStatement {
                statement: r#"
                    MATCH (n:GraphNode {id: $id})
                    OPTIONAL MATCH (n)-[r]-(m:GraphNode)
                    RETURN
                        {
                            id: n.id,
                            labels: labels(n),
                            properties: properties(n)
                        },
                        [rel IN collect(DISTINCT r) WHERE rel IS NOT NULL | {
                            id: elementId(rel),
                            source: startNode(rel).id,
                            target: endNode(rel).id,
                            type: type(rel),
                            properties: properties(rel)
                        }]
                "#
                .into(),
                parameters: json!({
                    "id": id,
                }),
            },
            "loading graph node details",
        )
        .await?;
        rows.first().map(graph_node_details_from_row).transpose()
    }

    /// Return the source audio and clip-local offsets for a speech segment.
    pub async fn graph_speech_segment_audio(
        &self,
        id: &str,
    ) -> Result<Option<GraphSpeechSegmentAudio>> {
        let endpoint = self.http_endpoint()?;
        let rows = query_neo4j_rows(
            &reqwest::Client::new(),
            &endpoint,
            &self.user,
            &self.pass,
            CypherStatement {
                statement: r#"
                    MATCH (s:GraphNode:SpeechSegment {id: $id})
                    CALL {
                        WITH s
                        MATCH (a:GraphNode:AudioClip {id: s.audio_clip_id})
                        WITH s, a,
                            toInteger(coalesce(s.start_ms, 0)) AS clip_start_ms,
                            toInteger(coalesce(s.end_ms, s.start_ms, 0)) AS clip_end_ms
                        RETURN a, clip_start_ms, clip_end_ms

                        UNION

                        WITH s
                        MATCH (s)<-[:HAS_SEGMENT]-(t:GraphNode:Transcription)
                        MATCH (s)-[source_rel:DERIVED_FROM_AUDIO]->(a:GraphNode:AudioClip)
                        OPTIONAL MATCH (t)-[out:DERIVED_FROM_AUDIO]->(a)
                        OPTIONAL MATCH (a)-[source_in:HAS_BIG_TRANSCRIPTION]->(t)
                        WITH s, a,
                            source_rel.clip_start_ms AS stored_clip_start_ms,
                            source_rel.clip_end_ms AS stored_clip_end_ms,
                            toInteger(coalesce(s.start_ms, 0)) AS segment_start_ms,
                            toInteger(coalesce(s.end_ms, s.start_ms, 0)) AS segment_end_ms,
                            toInteger(coalesce(source_rel.source_start_ms, out.start_ms, source_in.start_ms, 0)) AS source_start_ms,
                            toInteger(coalesce(source_rel.source_end_ms, out.end_ms, source_in.end_ms, s.end_ms, s.start_ms, 0)) AS source_end_ms
                        WHERE segment_start_ms < source_end_ms
                          AND segment_end_ms > source_start_ms
                        WITH s, a, source_start_ms, source_end_ms,
                            CASE
                                WHEN stored_clip_start_ms IS NOT NULL THEN toInteger(stored_clip_start_ms)
                                ELSE NULL
                            END AS stored_clip_start_ms,
                            CASE
                                WHEN stored_clip_end_ms IS NOT NULL THEN toInteger(stored_clip_end_ms)
                                ELSE NULL
                            END AS stored_clip_end_ms,
                            CASE
                                WHEN segment_start_ms > source_start_ms THEN segment_start_ms - source_start_ms
                                ELSE 0
                            END AS derived_clip_start_ms,
                            CASE
                                WHEN segment_end_ms < source_end_ms THEN segment_end_ms - source_start_ms
                                ELSE source_end_ms - source_start_ms
                            END AS derived_clip_end_ms
                        WITH s, a,
                            coalesce(stored_clip_start_ms, derived_clip_start_ms) AS clip_start_ms,
                            coalesce(stored_clip_end_ms, derived_clip_end_ms) AS clip_end_ms
                        RETURN a, clip_start_ms, clip_end_ms
                    }
                    RETURN
                        s.id,
                        coalesce(s.text, ""),
                        a.id,
                        a.mime,
                        a.base64,
                        a.sample_rate,
                        a.channels,
                        clip_start_ms,
                        clip_end_ms
                    ORDER BY clip_end_ms - clip_start_ms DESC, a.id
                    LIMIT 1
                "#
                .into(),
                parameters: json!({
                    "id": id,
                }),
            },
            "loading graph speech segment audio",
        )
        .await?;
        rows.first()
            .map(graph_speech_segment_audio_from_row)
            .transpose()
    }

    /// Return human-readable graph items represented by vector points.
    pub async fn vector_cluster_items(
        &self,
        collection: &str,
        point_ids: &[String],
        limit: usize,
    ) -> Result<Vec<GraphClusterItem>> {
        if point_ids.is_empty() {
            return Ok(Vec::new());
        }
        let vector_ids = point_ids
            .iter()
            .map(|point_id| qdrant_vector_node_id(collection, point_id))
            .collect::<Vec<_>>();
        let endpoint = self.http_endpoint()?;
        let rows = query_neo4j_rows(
            &reqwest::Client::new(),
            &endpoint,
            &self.user,
            &self.pass,
            CypherStatement {
                statement: r#"
                    UNWIND $vector_ids AS vector_id
                    MATCH (v:GraphNode:Vector {id: vector_id})
                    OPTIONAL MATCH (owner:GraphNode)-[owner_rel]->(v)
                    WHERE type(owner_rel) IN [
                        "HAS_MEMORY_VECTOR",
                        "HAS_IMAGE_DESCRIPTION_VECTOR",
                        "HAS_SCENE_VECTOR",
                        "HAS_FACE_VECTOR",
                        "HAS_GEOLOCATION_VECTOR",
                        "HAS_VOICE_VECTOR"
                    ]
                    WITH vector_id, v, collect(DISTINCT owner) AS direct_owners
                    UNWIND CASE WHEN size(direct_owners) = 0 THEN [v] ELSE direct_owners END AS owner
                    OPTIONAL MATCH (owner)-[:HAS_STIMULUS]->(:GraphNode:Stimulus)-[:REFERS_TO]->(stimulus:GraphNode)
                    WITH vector_id, owner, collect(DISTINCT stimulus) AS stimuli,
                    CASE
                        WHEN owner:SpeechSegment THEN "speech: " + coalesce(owner.text, "")
                        WHEN owner:Transcription THEN "transcription: " + coalesce(owner.text, owner.transcript, "")
                        WHEN owner:ImageDescription THEN "vision: " + coalesce(owner.text, "")
                        WHEN owner:Impression THEN "impression: " + coalesce(owner.summary, "")
                        WHEN owner:Awareness THEN "awareness: " + coalesce(owner.text, owner.summary, "")
                        WHEN owner:TextObservation THEN "text: " + coalesce(owner.text, "")
                        WHEN owner:Geolocation THEN "geolocation: " + toString(owner.latitude) + ", " + toString(owner.longitude)
                        WHEN owner:Heartbeat THEN "heartbeat"
                        WHEN owner:Image THEN "image captured"
                        WHEN owner:AudioClip THEN
                            CASE
                                WHEN owner.transcript IS NULL OR owner.transcript = "" THEN "audio clip captured"
                                ELSE "audio: " + owner.transcript
                            END
                        WHEN owner:ObjectObservation THEN "object: " + coalesce(owner.object_label, "unknown")
                        WHEN owner:Face THEN "face detected"
                        WHEN owner:Voice THEN "voice detected"
                        WHEN owner:Vector THEN "vector: " + coalesce(owner.collection, "") + "/" + coalesce(owner.point_id, "")
                        ELSE coalesce(owner.summary, owner.text, owner.transcript, owner.object_label, "")
                    END AS owner_text
                    WITH vector_id, owner, owner_text,
                    [stimulus IN stimuli WHERE stimulus IS NOT NULL |
                        CASE
                            WHEN stimulus:TextObservation THEN "text: " + coalesce(stimulus.text, "")
                            WHEN stimulus:Image THEN "image captured"
                            WHEN stimulus:AudioClip THEN
                                CASE
                                    WHEN stimulus.transcript IS NULL OR stimulus.transcript = "" THEN "audio clip captured"
                                    ELSE "audio: " + stimulus.transcript
                                END
                            WHEN stimulus:Geolocation THEN "geolocation: " + toString(stimulus.latitude) + ", " + toString(stimulus.longitude)
                            WHEN stimulus:ObjectObservation THEN "object: " + coalesce(stimulus.object_label, "unknown")
                            ELSE coalesce(stimulus.summary, stimulus.text, stimulus.transcript, stimulus.object_label, "")
                        END
                    ] AS stimulus_texts
                    WITH vector_id, owner,
                        CASE
                            WHEN owner_text <> "" THEN owner_text
                            ELSE head([text IN stimulus_texts WHERE text <> ""])
                        END AS text,
                        [text IN stimulus_texts WHERE text <> ""] AS stimulus_texts
                    CALL {
                        WITH owner
                        MATCH (owner)-[rel]-(neighbor:GraphNode)
                        WHERE NOT neighbor:Vector
                          AND NOT neighbor:Cluster
                          AND NOT neighbor:ClusterDiscoveryRun
                          AND NOT neighbor:ClusterThemeRun
                        WITH owner, rel, neighbor,
                            CASE
                                WHEN neighbor:SpeechSegment THEN "speech: " + coalesce(neighbor.text, "")
                                WHEN neighbor:Transcription THEN "transcription: " + coalesce(neighbor.text, neighbor.transcript, "")
                                WHEN neighbor:ImageDescription THEN "vision: " + coalesce(neighbor.text, "")
                                WHEN neighbor:Impression THEN "impression: " + coalesce(neighbor.summary, "")
                                WHEN neighbor:Awareness THEN "awareness: " + coalesce(neighbor.text, neighbor.summary, "")
                                WHEN neighbor:TextObservation THEN "text: " + coalesce(neighbor.text, "")
                                WHEN neighbor:Geolocation THEN "geolocation: " + toString(neighbor.latitude) + ", " + toString(neighbor.longitude)
                                WHEN neighbor:Heartbeat THEN "heartbeat"
                                WHEN neighbor:Image THEN "image captured"
                                WHEN neighbor:AudioClip THEN
                                    CASE
                                        WHEN neighbor.transcript IS NULL OR neighbor.transcript = "" THEN "audio clip captured"
                                        ELSE "audio: " + neighbor.transcript
                                    END
                                WHEN neighbor:ObjectObservation THEN "object: " + coalesce(neighbor.object_label, "unknown")
                                WHEN neighbor:Face THEN "face detected"
                                WHEN neighbor:Voice THEN "voice detected"
                                ELSE coalesce(neighbor.summary, neighbor.text, neighbor.transcript, neighbor.object_label, "")
                            END AS neighbor_text
                        ORDER BY type(rel), neighbor.id
                        WITH collect({
                            edge: (CASE
                                WHEN startNode(rel) = owner THEN "-[:" + type(rel) + "]-> "
                                ELSE "<-[:" + type(rel) + "]- "
                            END) + coalesce(neighbor.id, ""),
                            neighbor: coalesce(head([label IN labels(neighbor) WHERE label <> "GraphNode"]), "Node") + " " + CASE
                                WHEN neighbor_text <> "" THEN neighbor_text
                                ELSE coalesce(neighbor.id, "")
                            END
                        }) AS context
                        RETURN
                            [item IN context[..8] | item.edge] AS edge_texts,
                            [item IN context[..8] | item.neighbor] AS neighbor_texts
                    }
                    WITH vector_id, owner, text, stimulus_texts, edge_texts, neighbor_texts
                    WHERE text IS NOT NULL AND text <> ""
                    RETURN vector_id, owner.id, labels(owner), text, stimulus_texts, edge_texts, neighbor_texts
                    ORDER BY vector_id, owner.id
                    LIMIT $limit
                "#
                .into(),
                parameters: json!({
                    "vector_ids": vector_ids,
                    "limit": i64::try_from(limit.max(1)).unwrap_or(i64::MAX),
                }),
            },
            "loading vector cluster items",
        )
        .await?;
        rows.iter().map(graph_cluster_item_from_row).collect()
    }

    /// Return whether a cluster already has an attached LLM theme.
    pub async fn vector_cluster_has_theme(&self, cluster_id: &str) -> Result<bool> {
        let endpoint = self.http_endpoint()?;
        let rows = query_neo4j_rows(
            &reqwest::Client::new(),
            &endpoint,
            &self.user,
            &self.pass,
            CypherStatement {
                statement: r#"
                    MATCH (c:GraphNode:Cluster {id: $cluster_id})
                    RETURN EXISTS { MATCH (c)-[:HAS_THEME]->(:GraphNode:Theme) }
                "#
                .into(),
                parameters: json!({
                    "cluster_id": cluster_id,
                }),
            },
            "checking vector cluster theme",
        )
        .await?;
        Ok(rows
            .first()
            .and_then(Value::as_array)
            .and_then(|values| values.first())
            .and_then(Value::as_bool)
            .unwrap_or(false))
    }

    /// Return a recent graph timeline for an offline combobulation pass.
    ///
    /// The anchor is the newest displayable graph event that has not already
    /// been included in a combobulation run. The returned window includes that
    /// anchor plus earlier events from the requested lookback period.
    pub async fn latest_timeline_window_for_combobulation(
        &self,
        seconds: u64,
        limit: usize,
    ) -> Result<Option<GraphTimelineWindow>> {
        let endpoint = self.http_endpoint()?;
        let rows = query_neo4j_rows(
            &reqwest::Client::new(),
            &endpoint,
            &self.user,
            &self.pass,
            CypherStatement {
                statement: r#"
                    MATCH (anchor:GraphNode:Sensation)
                    WHERE coalesce(anchor.occurred_at, "") <> ""
                      AND NOT (anchor)-[:INCLUDED_IN_COMBOBULATION]->(:GraphNode:CombobulationRun)
                    WITH anchor, anchor.occurred_at AS anchor_at
                    ORDER BY datetime(anchor_at) DESC, anchor.id
                    LIMIT 1
                    MATCH (n:GraphNode:Sensation)
                    WHERE coalesce(n.occurred_at, "") <> ""
                    WITH anchor, anchor_at, n, n.occurred_at AS occurred_at
                    WHERE datetime(occurred_at) >= datetime(anchor_at) - duration({seconds: $seconds})
                      AND datetime(occurred_at) <= datetime(anchor_at)
                    OPTIONAL MATCH (n)-[:OBSERVED|PRODUCED]->(artifact:GraphNode)
                    OPTIONAL MATCH (artifact)-[:HAS_TRANSCRIPTION|HAS_BIG_TRANSCRIPTION]->(attached_transcription:GraphNode:Transcription)
                    OPTIONAL MATCH (artifact)-[:HAS_IMAGE_DESCRIPTION]->(attached_description:GraphNode:ImageDescription)
                    WITH anchor, anchor_at, n, occurred_at,
                        collect(DISTINCT artifact) AS artifacts,
                        head([t IN collect(DISTINCT attached_transcription)
                            WHERE coalesce(t.text, t.transcript, "") <> "" |
                            coalesce(t.text, t.transcript, "")
                        ]) AS attached_transcript,
                        head([d IN collect(DISTINCT attached_description)
                            WHERE coalesce(d.text, "") <> "" |
                            d.text
                        ]) AS image_description
                    WITH anchor, anchor_at, n, occurred_at,
                        head([a IN artifacts WHERE a:AudioClip | a]) AS audio,
                        head([a IN artifacts WHERE a:Image | a]) AS image,
                        head([a IN artifacts WHERE a:Geolocation | a]) AS geolocation,
                        head([a IN artifacts WHERE a:Heartbeat | a]) AS heartbeat,
                        head([a IN artifacts WHERE a:ObjectObservation | a]) AS object,
                        head([a IN artifacts WHERE a:Utterance | a]) AS utterance,
                        head([a IN artifacts WHERE a:CombobulationSummary | a]) AS combobulation,
                        head([a IN artifacts WHERE a:JsonSensation | a]) AS json_sensation,
                        attached_transcript,
                        image_description
                    WITH anchor, anchor_at, n, occurred_at,
                    coalesce(
                        audio.id,
                        image.id,
                        geolocation.id,
                        heartbeat.id,
                        object.id,
                        utterance.id,
                        combobulation.id,
                        json_sensation.id,
                        n.id
                    ) AS event_id,
                    CASE
                        WHEN audio IS NOT NULL THEN
                            CASE
                                WHEN attached_transcript IS NOT NULL AND attached_transcript <> "" THEN
                                    "audio sensation; transcript: " + attached_transcript
                                WHEN audio.transcript IS NULL OR audio.transcript = "" THEN
                                    "audio sensation; transcript pending"
                                ELSE "audio sensation; transcript: " + audio.transcript
                            END
                        WHEN image_description IS NOT NULL AND image_description <> "" THEN
                            "visual sensation; " + image_description
                        WHEN image IS NOT NULL THEN "visual sensation; image captured"
                        WHEN geolocation IS NOT NULL THEN
                            "geolocation sensation; " + toString(geolocation.latitude) + ", " + toString(geolocation.longitude)
                        WHEN heartbeat IS NOT NULL THEN "heartbeat sensation"
                        WHEN object IS NOT NULL THEN
                            "object sensation; " + coalesce(object.object_label, "unknown")
                        WHEN utterance IS NOT NULL THEN
                            "speech sensation; " + coalesce(utterance.speaker, "someone") + " said: " + coalesce(utterance.text, "")
                        WHEN combobulation IS NOT NULL THEN
                            "combobulation sensation; " + coalesce(combobulation.text, combobulation.summary, "")
                        WHEN json_sensation IS NOT NULL THEN "structured sensation"
                        ELSE coalesce(n.kind, "sensation")
                    END AS text
                    WHERE text <> ""
                    WITH anchor, anchor_at, n, occurred_at, event_id, text
                    ORDER BY datetime(occurred_at) ASC, n.id
                    LIMIT $limit
                    RETURN anchor.id, anchor_at, n.id, event_id, labels(n), text, occurred_at
                "#
                .into(),
                parameters: json!({
                    "seconds": i64::try_from(seconds.max(1)).unwrap_or(i64::MAX),
                    "limit": i64::try_from(limit.max(1)).unwrap_or(i64::MAX),
                }),
            },
            "finding latest timeline window for combobulation",
        )
        .await?;
        graph_timeline_window_from_rows(&rows)
    }

    /// Attach a Whisper transcript to an existing `AudioClip` graph node.
    pub async fn attach_audio_transcription(
        &self,
        audio_clip_id: &str,
        transcript: &str,
        source_captured_at: Option<&str>,
        segments: &[GraphSpeechSegment],
    ) -> Result<()> {
        let endpoint = self.http_endpoint()?;
        let client = reqwest::Client::new();
        self.ensure_constraint(&client, &endpoint).await?;
        let transcribed_at = chrono::Utc::now().to_rfc3339();
        let transcription_id = stable_bytes_id(
            "transcription",
            format!("{audio_clip_id}:{transcribed_at}").as_bytes(),
        );
        let mut nodes = vec![
            json!({
                "label": "AudioClip",
                "id": audio_clip_id,
            }),
            json!({
                "label": "Transcription",
                "id": transcription_id,
                "audio_clip_id": audio_clip_id,
                "text": transcript,
                "transcribed_at": transcribed_at,
                "source_captured_at": source_captured_at,
            }),
        ];
        let mut relationships = vec![
            json!({
                "from": audio_clip_id,
                "to": transcription_id,
                "type": "HAS_TRANSCRIPTION",
            }),
            json!({
                "from": transcription_id,
                "to": audio_clip_id,
                "type": "DERIVED_FROM_AUDIO",
            }),
        ];
        for segment in segments {
            let segment_id = format!("{transcription_id}:segment:{}", segment.index);
            nodes.push(json!({
                "label": "SpeechSegment",
                "id": segment_id,
                "transcription_id": transcription_id,
                "audio_clip_id": audio_clip_id,
                "segment_index": segment.index,
                "text": segment.text,
                "start_ms": segment.start_ms,
                "end_ms": segment.end_ms,
                "occurred_at": segment.occurred_at,
                "ended_at": segment.ended_at,
            }));
            relationships.push(json!({
                "from": transcription_id,
                "to": segment_id,
                "type": "HAS_SEGMENT",
                "segment_index": segment.index,
            }));
        }
        let mut statements = graph_statements(&json!({
            "op": "merge_graph",
            "nodes": nodes,
            "relationships": relationships,
        }))?;
        statements.push(CypherStatement {
            statement: r#"
                MATCH (a:GraphNode:AudioClip {id: $id})
                SET a.transcript = $transcript,
                    a.transcribed_at = $transcribed_at
            "#
            .into(),
            parameters: json!({
                "id": audio_clip_id,
                "transcript": transcript,
                "transcribed_at": transcribed_at,
            }),
        });
        commit_neo4j_statements(
            &client,
            &endpoint,
            &self.user,
            &self.pass,
            &statements,
            "attaching audio transcription",
        )
        .await
    }

    /// Attach one aggregate Whisper transcript to several source audio clips.
    pub async fn attach_big_audio_transcription(
        &self,
        sources: &[GraphAudioSourceSpan],
        transcript: &str,
        source_started_at: Option<&str>,
        source_ended_at: Option<&str>,
        segments: &[GraphSpeechSegment],
    ) -> Result<()> {
        anyhow::ensure!(!sources.is_empty(), "big transcription has no source clips");
        let endpoint = self.http_endpoint()?;
        let client = reqwest::Client::new();
        self.ensure_constraint(&client, &endpoint).await?;
        let transcribed_at = chrono::Utc::now().to_rfc3339();
        let source_ids = sources
            .iter()
            .map(|source| source.audio_clip_id.clone())
            .collect::<Vec<_>>();
        let transcription_id = stable_bytes_id(
            "big-transcription",
            format!("{}:{transcribed_at}", source_ids.join(",")).as_bytes(),
        );
        let mut nodes = vec![json!({
            "label": "Transcription",
            "id": transcription_id,
            "kind": "big",
            "audio_clip_ids": source_ids,
            "source_count": sources.len(),
            "text": transcript,
            "transcript": transcript,
            "transcribed_at": transcribed_at,
            "source_started_at": source_started_at,
            "source_ended_at": source_ended_at,
        })];
        let mut relationships = Vec::new();
        for source in sources {
            nodes.push(json!({
                "label": "AudioClip",
                "id": source.audio_clip_id,
            }));
            if let Some(sensation_id) = &source.sensation_id {
                nodes.push(json!({
                    "label": "Sensation",
                    "id": sensation_id,
                }));
                relationships.push(json!({
                    "from": sensation_id,
                    "to": transcription_id,
                    "type": "PRODUCED",
                    "source_index": source.index,
                    "anchor": source.anchor,
                }));
            }
            relationships.push(json!({
                "from": source.audio_clip_id,
                "to": transcription_id,
                "type": "HAS_BIG_TRANSCRIPTION",
                "source_index": source.index,
                "start_ms": source.start_ms,
                "end_ms": source.end_ms,
                "occurred_at": source.occurred_at,
                "ended_at": source.ended_at,
                "anchor": source.anchor,
            }));
            relationships.push(json!({
                "from": transcription_id,
                "to": source.audio_clip_id,
                "type": "DERIVED_FROM_AUDIO",
                "source_index": source.index,
                "start_ms": source.start_ms,
                "end_ms": source.end_ms,
                "occurred_at": source.occurred_at,
                "ended_at": source.ended_at,
                "anchor": source.anchor,
            }));
        }
        for segment in segments {
            let segment_id = format!("{transcription_id}:segment:{}", segment.index);
            nodes.push(json!({
                "label": "SpeechSegment",
                "id": segment_id,
                "transcription_id": transcription_id,
                "segment_index": segment.index,
                "text": segment.text,
                "start_ms": segment.start_ms,
                "end_ms": segment.end_ms,
                "occurred_at": segment.occurred_at,
                "ended_at": segment.ended_at,
            }));
            relationships.push(json!({
                "from": transcription_id,
                "to": segment_id,
                "type": "HAS_SEGMENT",
                "segment_index": segment.index,
            }));
            for source in sources.iter().filter(|source| {
                spans_overlap(
                    segment.start_ms,
                    segment.end_ms,
                    source.start_ms,
                    source.end_ms,
                )
            }) {
                let (clip_start_ms, clip_end_ms) = clip_local_overlap(
                    segment.start_ms,
                    segment.end_ms,
                    source.start_ms,
                    source.end_ms,
                );
                relationships.push(json!({
                    "from": segment_id,
                    "to": source.audio_clip_id,
                    "type": "DERIVED_FROM_AUDIO",
                    "source_index": source.index,
                    "segment_start_ms": segment.start_ms,
                    "segment_end_ms": segment.end_ms,
                    "source_start_ms": source.start_ms,
                    "source_end_ms": source.end_ms,
                    "clip_start_ms": clip_start_ms,
                    "clip_end_ms": clip_end_ms,
                }));
            }
        }
        let statements = graph_statements(&json!({
            "op": "merge_graph",
            "nodes": nodes,
            "relationships": relationships,
        }))?;
        commit_neo4j_statements(
            &client,
            &endpoint,
            &self.user,
            &self.pass,
            &statements,
            "attaching big audio transcription",
        )
        .await
    }

    /// Attach one fused audio clip to an existing big transcription and optionally remove its source subgraph.
    pub async fn consolidate_big_audio_transcription(
        &self,
        candidate: &GraphConsolidatedSpeechCandidate,
        consolidated_clip_id: &str,
        consolidated_clip: &AudioClip,
        duration_ms: u32,
        delete_subnodes: bool,
    ) -> Result<GraphSpeechConsolidationReport> {
        anyhow::ensure!(
            !candidate.sources.is_empty(),
            "speech consolidation has no source clips"
        );
        let endpoint = self.http_endpoint()?;
        let client = reqwest::Client::new();
        self.ensure_constraint(&client, &endpoint).await?;
        let consolidated_at = chrono::Utc::now().to_rfc3339();
        let source_audio_ids = candidate
            .sources
            .iter()
            .map(|source| source.clip.id.clone())
            .collect::<Vec<_>>();
        let source_sensation_ids = candidate
            .sources
            .iter()
            .filter_map(|source| source.clip.sensation_id.clone())
            .collect::<Vec<_>>();
        let old_transcription_ids = candidate
            .sources
            .iter()
            .flat_map(|source| source.transcription_ids.iter().cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();

        let graph = json!({
            "op": "merge_graph",
            "nodes": [
                {
                    "label": "AudioClip",
                    "id": consolidated_clip_id,
                    "mime": consolidated_clip.mime,
                    "base64": consolidated_clip.base64,
                    "sample_rate": consolidated_clip.sample_rate,
                    "channels": consolidated_clip.channels,
                    "transcript": consolidated_clip.transcript,
                    "captured_at": consolidated_clip.captured_at,
                    "occurred_at": consolidated_clip.captured_at,
                    "source_count": candidate.sources.len(),
                    "source_audio_clip_ids": source_audio_ids,
                    "consolidated_at": consolidated_at,
                },
                {
                    "label": "Transcription",
                    "id": candidate.transcription_id,
                    "consolidated_audio_clip_id": consolidated_clip_id,
                    "consolidated_at": consolidated_at,
                }
            ],
            "relationships": [
                {
                    "from": candidate.transcription_id,
                    "to": consolidated_clip_id,
                    "type": "HAS_CONSOLIDATED_AUDIO",
                    "duration_ms": duration_ms,
                    "source_count": candidate.sources.len(),
                },
                {
                    "from": consolidated_clip_id,
                    "to": candidate.transcription_id,
                    "type": "HAS_BIG_TRANSCRIPTION",
                    "source_index": 0,
                    "start_ms": 0,
                    "end_ms": duration_ms,
                    "anchor": true,
                },
                {
                    "from": candidate.transcription_id,
                    "to": consolidated_clip_id,
                    "type": "DERIVED_FROM_AUDIO",
                    "source_index": 0,
                    "start_ms": 0,
                    "end_ms": duration_ms,
                    "anchor": true,
                }
            ],
        });
        let mut statements = graph_statements(&graph)?;
        statements.push(CypherStatement {
            statement: r#"
                MATCH (t:GraphNode:Transcription {id: $transcription_id})-[:HAS_SEGMENT]->(segment:GraphNode:SpeechSegment)
                MATCH (a:GraphNode:AudioClip {id: $consolidated_audio_clip_id})
                WITH segment, a,
                    toInteger(coalesce(segment.start_ms, 0)) AS start_ms,
                    toInteger(coalesce(segment.end_ms, segment.start_ms, 0)) AS end_ms
                MERGE (segment)-[rel:DERIVED_FROM_AUDIO]->(a)
                SET rel.source_index = 0,
                    rel.segment_start_ms = start_ms,
                    rel.segment_end_ms = end_ms,
                    rel.source_start_ms = 0,
                    rel.source_end_ms = $duration_ms,
                    rel.clip_start_ms = start_ms,
                    rel.clip_end_ms = end_ms
            "#
            .into(),
            parameters: json!({
                "transcription_id": candidate.transcription_id,
                "consolidated_audio_clip_id": consolidated_clip_id,
                "duration_ms": i64::from(duration_ms),
            }),
        });
        if delete_subnodes {
            statements.push(CypherStatement {
                statement: r#"
                    MATCH (old:GraphNode:Transcription)
                    WHERE old.id IN $old_transcription_ids
                    MATCH (old)-[:HAS_SEGMENT]->(segment:GraphNode:SpeechSegment)
                    DETACH DELETE segment
                "#
                .into(),
                parameters: json!({
                    "old_transcription_ids": old_transcription_ids,
                }),
            });
            statements.push(CypherStatement {
                statement: r#"
                    MATCH (old:GraphNode:Transcription)
                    WHERE old.id IN $old_transcription_ids
                    DETACH DELETE old
                "#
                .into(),
                parameters: json!({
                    "old_transcription_ids": old_transcription_ids,
                }),
            });
            statements.push(CypherStatement {
                statement: r#"
                    MATCH (a:GraphNode:AudioClip)
                    WHERE a.id IN $source_audio_ids
                    DETACH DELETE a
                "#
                .into(),
                parameters: json!({
                    "source_audio_ids": source_audio_ids,
                }),
            });
            statements.push(CypherStatement {
                statement: r#"
                    MATCH (s:GraphNode:Sensation)
                    WHERE s.id IN $source_sensation_ids
                      AND NOT EXISTS { MATCH (s)--(:GraphNode) }
                    DETACH DELETE s
                "#
                .into(),
                parameters: json!({
                    "source_sensation_ids": source_sensation_ids,
                }),
            });
        }
        commit_neo4j_statements(
            &client,
            &endpoint,
            &self.user,
            &self.pass,
            &statements,
            "consolidating big audio transcription",
        )
        .await?;
        Ok(GraphSpeechConsolidationReport {
            transcription_id: candidate.transcription_id.clone(),
            consolidated_audio_clip_id: consolidated_clip_id.to_string(),
            source_audio_clip_ids: source_audio_ids,
            deleted_transcription_ids: if delete_subnodes {
                old_transcription_ids
            } else {
                Vec::new()
            },
        })
    }

    /// Attach face recognition results to an existing `Image` graph node.
    pub async fn attach_face_recognition(
        &self,
        frame: &GraphImageFrame,
        detector: &str,
        detections: &[GraphFaceDetection],
    ) -> Result<()> {
        let processed_at = chrono::Utc::now().to_rfc3339();
        let run_id = format!("face-recognition:{}", frame.id);
        let mut nodes = vec![
            json!({
                "label": "Image",
                "id": frame.id,
            }),
            json!({
                "label": "FaceRecognitionRun",
                "id": run_id,
                "image_id": frame.id,
                "detector": detector,
                "processed_at": processed_at,
                "face_count": detections.len(),
            }),
        ];
        let mut relationships = vec![
            json!({
                "from": frame.id,
                "to": run_id,
                "type": "HAS_FACE_RECOGNITION_RUN",
            }),
            json!({
                "from": run_id,
                "to": frame.id,
                "type": "PROCESSED_IMAGE",
            }),
        ];

        if let Some(sensation_id) = &frame.sensation_id {
            nodes.push(json!({
                "label": "Sensation",
                "id": sensation_id,
            }));
            relationships.push(json!({
                "from": sensation_id,
                "to": run_id,
                "type": "PRODUCED",
            }));
        }

        for detection in detections {
            let vector_id = qdrant_vector_node_id(FACE_COLLECTION, &detection.vector_id);
            nodes.push(json!({
                "label": "Face",
                "id": detection.face_id,
                "source_image_id": frame.id,
                "crop_mime": detection.crop.mime.clone(),
                "crop_base64": detection.crop.base64.clone(),
                "captured_at": detection.crop.captured_at.clone(),
                "occurred_at": detection
                    .crop
                    .captured_at
                    .clone()
                    .or_else(|| frame.occurred_at.clone()),
                "detection_index": detection.index,
                "embedding_len": detection.embedding_len,
                "recognized_at": processed_at,
            }));
            nodes.push(qdrant_vector_node(
                FACE_COLLECTION,
                &detection.vector_id,
                "face",
                Some(detector),
            ));
            relationships.push(json!({
                "from": run_id,
                "to": detection.face_id,
                "type": "DETECTED_FACE",
                "detection_index": detection.index,
            }));
            relationships.push(json!({
                "from": frame.id,
                "to": detection.face_id,
                "type": "CONTAINS_FACE",
            }));
            relationships.push(json!({
                "from": detection.face_id,
                "to": frame.id,
                "type": "DERIVED_FROM",
            }));
            relationships.push(json!({
                "from": detection.face_id,
                "to": vector_id,
                "type": "HAS_FACE_VECTOR",
            }));
            relationships.push(json!({
                "from": run_id,
                "to": vector_id,
                "type": "PRODUCED",
            }));
            if let Some(sensation_id) = &frame.sensation_id {
                relationships.push(json!({
                    "from": sensation_id,
                    "to": detection.face_id,
                    "type": "PRODUCED",
                }));
                relationships.push(json!({
                    "from": sensation_id,
                    "to": vector_id,
                    "type": "PRODUCED",
                }));
            }
        }

        self.store_data(&json!({
            "op": "merge_graph",
            "nodes": nodes,
            "relationships": relationships,
        }))
        .await
    }

    /// Attach scene vectorization results to an existing `Image` graph node.
    pub async fn attach_scene_vectorization(
        &self,
        frame: &GraphImageFrame,
        model: &str,
        scene: &GraphSceneVectorization,
    ) -> Result<()> {
        let processed_at = chrono::Utc::now().to_rfc3339();
        let run_id = format!("scene-vectorization:{}", frame.id);
        let vector_id = qdrant_vector_node_id(SCENE_VECTOR_COLLECTION, &scene.vector_id);
        let mut nodes = vec![
            json!({
                "label": "Image",
                "id": frame.id,
            }),
            json!({
                "label": "SceneVectorizationRun",
                "id": run_id,
                "image_id": frame.id,
                "model": model,
                "processed_at": processed_at,
                "embedding_len": scene.embedding_len,
            }),
            qdrant_vector_node(
                SCENE_VECTOR_COLLECTION,
                &scene.vector_id,
                "scene",
                Some(model),
            ),
        ];
        let mut relationships = vec![
            json!({
                "from": frame.id,
                "to": run_id,
                "type": "HAS_SCENE_VECTORIZATION_RUN",
            }),
            json!({
                "from": run_id,
                "to": frame.id,
                "type": "PROCESSED_IMAGE",
            }),
            json!({
                "from": frame.id,
                "to": vector_id,
                "type": "HAS_SCENE_VECTOR",
            }),
            json!({
                "from": vector_id,
                "to": frame.id,
                "type": "DERIVED_FROM",
            }),
            json!({
                "from": run_id,
                "to": vector_id,
                "type": "PRODUCED",
            }),
        ];

        if let Some(sensation_id) = &frame.sensation_id {
            nodes.push(json!({
                "label": "Sensation",
                "id": sensation_id,
            }));
            relationships.push(json!({
                "from": sensation_id,
                "to": run_id,
                "type": "PRODUCED",
            }));
            relationships.push(json!({
                "from": sensation_id,
                "to": vector_id,
                "type": "PRODUCED",
            }));
        }

        self.store_data(&json!({
            "op": "merge_graph",
            "nodes": nodes,
            "relationships": relationships,
        }))
        .await
    }

    /// Attach an LLM image description and its text embedding to an existing `Image`.
    pub async fn attach_image_description(
        &self,
        frame: &GraphImageFrame,
        vision_model: &str,
        embedding_model: &str,
        description: &GraphImageDescription,
    ) -> Result<()> {
        let processed_at = chrono::Utc::now().to_rfc3339();
        let run_id = format!("image-description:{}", frame.id);
        let vector_id = qdrant_vector_node_id(IMAGE_DESCRIPTION_COLLECTION, &description.vector_id);
        let mut nodes = vec![
            json!({
                "label": "Image",
                "id": frame.id,
            }),
            json!({
                "label": "ImageDescriptionRun",
                "id": run_id,
                "image_id": frame.id,
                "model": vision_model,
                "embedding_model": embedding_model,
                "processed_at": processed_at,
                "embedding_len": description.embedding_len,
            }),
            json!({
                "label": "ImageDescription",
                "id": description.description_id,
                "image_id": frame.id,
                "text": description.text,
                "model": vision_model,
                "described_at": processed_at,
                "occurred_at": frame
                    .image
                    .captured_at
                    .clone()
                    .or_else(|| frame.occurred_at.clone()),
            }),
            qdrant_vector_node(
                IMAGE_DESCRIPTION_COLLECTION,
                &description.vector_id,
                "image_description",
                Some(embedding_model),
            ),
        ];
        let mut relationships = vec![
            json!({
                "from": frame.id,
                "to": run_id,
                "type": "HAS_IMAGE_DESCRIPTION_RUN",
            }),
            json!({
                "from": run_id,
                "to": frame.id,
                "type": "PROCESSED_IMAGE",
            }),
            json!({
                "from": run_id,
                "to": description.description_id,
                "type": "PRODUCED",
            }),
            json!({
                "from": frame.id,
                "to": description.description_id,
                "type": "HAS_IMAGE_DESCRIPTION",
            }),
            json!({
                "from": description.description_id,
                "to": frame.id,
                "type": "DERIVED_FROM",
            }),
            json!({
                "from": description.description_id,
                "to": vector_id,
                "type": "HAS_IMAGE_DESCRIPTION_VECTOR",
            }),
            json!({
                "from": frame.id,
                "to": vector_id,
                "type": "HAS_IMAGE_DESCRIPTION_VECTOR",
            }),
            json!({
                "from": vector_id,
                "to": description.description_id,
                "type": "DERIVED_FROM",
            }),
            json!({
                "from": run_id,
                "to": vector_id,
                "type": "PRODUCED",
            }),
        ];

        if let Some(sensation_id) = &frame.sensation_id {
            nodes.push(json!({
                "label": "Sensation",
                "id": sensation_id,
            }));
            relationships.push(json!({
                "from": sensation_id,
                "to": run_id,
                "type": "PRODUCED",
            }));
            relationships.push(json!({
                "from": sensation_id,
                "to": description.description_id,
                "type": "PRODUCED",
            }));
            relationships.push(json!({
                "from": sensation_id,
                "to": vector_id,
                "type": "PRODUCED",
            }));
        }

        self.store_data(&json!({
            "op": "merge_graph",
            "nodes": nodes,
            "relationships": relationships,
        }))
        .await
    }

    /// Attach an offline combobulation summary and its text embedding to source events.
    pub async fn attach_combobulation(
        &self,
        window: &GraphTimelineWindow,
        llm_model: &str,
        embedding_model: &str,
        awareness: &GraphAwareness,
    ) -> Result<()> {
        anyhow::ensure!(
            !window.items.is_empty(),
            "combobulation has no source timeline items"
        );
        let processed_at = chrono::Utc::now().to_rfc3339();
        let run_id = stable_bytes_id(
            "combobulation",
            format!("{}:{processed_at}", window.anchor_id).as_bytes(),
        );
        let source_ids = window
            .items
            .iter()
            .map(|item| item.id.clone())
            .collect::<Vec<_>>();
        let source_started_at = window.items.first().map(|item| item.occurred_at.clone());
        let source_ended_at = window.items.last().map(|item| item.occurred_at.clone());
        let vector_id = qdrant_vector_node_id(MEMORY_COLLECTION, &awareness.vector_id);
        let nodes = vec![
            json!({
                "label": "CombobulationRun",
                "id": run_id,
                "anchor_id": window.anchor_id,
                "anchor_at": window.anchor_at,
                "model": llm_model,
                "embedding_model": embedding_model,
                "processed_at": processed_at,
                "source_count": window.items.len(),
                "source_ids": source_ids,
                "source_started_at": source_started_at,
                "source_ended_at": source_ended_at,
                "embedding_len": awareness.embedding_len,
            }),
            json!({
                "label": "Awareness",
                "id": awareness.awareness_id,
                "summary": awareness.text,
                "text": awareness.text,
                "model": llm_model,
                "embedding_model": embedding_model,
                "occurred_at": source_ended_at,
                "created_at": processed_at,
            }),
            qdrant_vector_node(
                MEMORY_COLLECTION,
                &awareness.vector_id,
                "memory",
                Some(embedding_model),
            ),
        ];
        let mut relationships = vec![
            json!({
                "from": run_id,
                "to": awareness.awareness_id,
                "type": "PRODUCED",
            }),
            json!({
                "from": awareness.awareness_id,
                "to": vector_id,
                "type": "HAS_MEMORY_VECTOR",
            }),
            json!({
                "from": run_id,
                "to": vector_id,
                "type": "PRODUCED",
            }),
        ];

        for (index, item) in window.items.iter().enumerate() {
            relationships.push(json!({
                "from": item.id,
                "to": run_id,
                "type": "INCLUDED_IN_COMBOBULATION",
                "source_index": index,
                "occurred_at": item.occurred_at,
            }));
            relationships.push(json!({
                "from": awareness.awareness_id,
                "to": item.id,
                "type": "DERIVED_FROM",
                "source_index": index,
                "occurred_at": item.occurred_at,
            }));
        }

        self.store_data(&json!({
            "op": "merge_graph",
            "nodes": nodes,
            "relationships": relationships,
        }))
        .await
    }

    /// Attach an LLM-generated theme to a discovered vector cluster.
    pub async fn attach_vector_cluster_theme(
        &self,
        cluster: &VectorCluster,
        llm_model: &str,
        items: &[GraphClusterItem],
        theme: &GraphClusterTheme,
    ) -> Result<()> {
        anyhow::ensure!(!items.is_empty(), "cluster theme has no source items");
        let processed_at = chrono::Utc::now().to_rfc3339();
        let run_id = stable_bytes_id(
            "cluster-theme-run",
            format!("{}:{llm_model}:{processed_at}", cluster.cluster_id).as_bytes(),
        );
        let source_ids = items
            .iter()
            .map(|item| item.node_id.clone())
            .collect::<Vec<_>>();
        let mut nodes = vec![
            json!({
                "label": "Cluster",
                "id": cluster.cluster_id,
                "collection": cluster.collection,
                "threshold": cluster.threshold,
                "member_count": cluster.members.len(),
                "mean_similarity": cluster.mean_similarity,
                "centroid_len": cluster.centroid.len(),
            }),
            json!({
                "label": "ClusterThemeRun",
                "id": run_id,
                "cluster_id": cluster.cluster_id,
                "model": llm_model,
                "source_count": items.len(),
                "source_ids": source_ids,
                "processed_at": processed_at,
            }),
            json!({
                "label": "Theme",
                "id": theme.theme_id,
                "text": theme.text,
                "summary": theme.text,
                "model": llm_model,
                "source_count": items.len(),
                "cluster_id": cluster.cluster_id,
                "created_at": processed_at,
            }),
        ];
        let mut relationships = vec![
            json!({
                "from": run_id,
                "to": theme.theme_id,
                "type": "PRODUCED",
            }),
            json!({
                "from": cluster.cluster_id,
                "to": run_id,
                "type": "HAS_CLUSTER_THEME_RUN",
            }),
            json!({
                "from": cluster.cluster_id,
                "to": theme.theme_id,
                "type": "HAS_THEME",
            }),
            json!({
                "from": theme.theme_id,
                "to": cluster.cluster_id,
                "type": "THEME_OF",
            }),
        ];

        for (index, item) in items.iter().enumerate() {
            relationships.push(json!({
                "from": theme.theme_id,
                "to": item.node_id,
                "type": "DERIVED_FROM",
                "source_index": index,
                "vector_id": item.vector_id,
            }));
            relationships.push(json!({
                "from": item.node_id,
                "to": run_id,
                "type": "INCLUDED_IN_CLUSTER_THEME",
                "source_index": index,
                "vector_id": item.vector_id,
            }));
            if item.vector_id.starts_with("qdrant:") {
                relationships.push(json!({
                    "from": theme.theme_id,
                    "to": item.vector_id,
                    "type": "DERIVED_FROM_VECTOR",
                    "source_index": index,
                }));
                nodes.push(json!({
                    "label": "Vector",
                    "id": item.vector_id,
                }));
            }
        }

        self.store_data(&json!({
            "op": "merge_graph",
            "nodes": nodes,
            "relationships": relationships,
        }))
        .await
    }

    /// Attach vector cluster discovery output to existing Qdrant vector nodes.
    pub async fn attach_vector_clusters(
        &self,
        collection: &str,
        algorithm: &str,
        threshold: f32,
        min_size: usize,
        source_count: usize,
        clusters: &[VectorCluster],
    ) -> Result<()> {
        let processed_at = chrono::Utc::now().to_rfc3339();
        let run_id = stable_bytes_id(
            "cluster-discovery",
            format!("{collection}:{algorithm}:{threshold:.4}:{min_size}:{processed_at}").as_bytes(),
        );
        let mut nodes = vec![json!({
            "label": "ClusterDiscoveryRun",
            "id": run_id,
            "collection": collection,
            "algorithm": algorithm,
            "threshold": threshold,
            "min_size": min_size,
            "source_count": source_count,
            "cluster_count": clusters.len(),
            "processed_at": processed_at,
        })];
        let mut relationships = Vec::new();

        for (cluster_index, cluster) in clusters.iter().enumerate() {
            let member_ids = cluster
                .members
                .iter()
                .map(|member| member.point_id.clone())
                .collect::<Vec<_>>();
            nodes.push(json!({
                "label": "Cluster",
                "id": cluster.cluster_id,
                "collection": cluster.collection,
                "algorithm": algorithm,
                "threshold": cluster.threshold,
                "member_count": cluster.members.len(),
                "member_ids": member_ids,
                "mean_similarity": cluster.mean_similarity,
                "centroid_len": cluster.centroid.len(),
            }));
            relationships.push(json!({
                "from": run_id,
                "to": cluster.cluster_id,
                "type": "PRODUCED_CLUSTER",
                "cluster_index": cluster_index,
            }));
            for member in &cluster.members {
                let vector_id = qdrant_vector_node_id(collection, &member.point_id);
                nodes.push(qdrant_vector_node(
                    collection,
                    &member.point_id,
                    collection,
                    None,
                ));
                relationships.push(json!({
                    "from": cluster.cluster_id,
                    "to": vector_id,
                    "type": "HAS_CLUSTER_MEMBER",
                    "average_similarity": member.average_similarity,
                }));
                relationships.push(json!({
                    "from": vector_id,
                    "to": cluster.cluster_id,
                    "type": "MEMBER_OF_CLUSTER",
                    "average_similarity": member.average_similarity,
                }));
            }
        }

        self.store_data(&json!({
            "op": "merge_graph",
            "nodes": nodes,
            "relationships": relationships,
        }))
        .await
    }

    /// Mark an `AudioClip` as attempted when voice recognition cannot use it.
    pub async fn attach_skipped_voice_recognition(
        &self,
        clip: &GraphVoiceClip,
        model: &str,
        reason: &str,
    ) -> Result<()> {
        let processed_at = chrono::Utc::now().to_rfc3339();
        let run_id = format!("voice-recognition:{}", clip.id);
        let mut nodes = vec![
            json!({
                "label": "AudioClip",
                "id": clip.id,
            }),
            json!({
                "label": "VoiceRecognitionRun",
                "id": run_id,
                "audio_clip_id": clip.id,
                "model": model,
                "processed_at": processed_at,
                "status": "skipped",
                "reason": reason,
            }),
        ];
        let mut relationships = vec![
            json!({
                "from": clip.id,
                "to": run_id,
                "type": "HAS_VOICE_RECOGNITION_RUN",
            }),
            json!({
                "from": run_id,
                "to": clip.id,
                "type": "PROCESSED_AUDIO",
            }),
        ];

        if let Some(sensation_id) = &clip.sensation_id {
            nodes.push(json!({
                "label": "Sensation",
                "id": sensation_id,
            }));
            relationships.push(json!({
                "from": sensation_id,
                "to": run_id,
                "type": "PRODUCED",
            }));
        }

        self.store_data(&json!({
            "op": "merge_graph",
            "nodes": nodes,
            "relationships": relationships,
        }))
        .await
    }

    /// Attach geolocation vectorization results to an existing `Geolocation` graph node.
    pub async fn attach_geolocation_vectorization(
        &self,
        geolocation: &GraphGeolocation,
        model: &str,
        vector_id: &str,
        embedding_len: usize,
    ) -> Result<()> {
        let processed_at = chrono::Utc::now().to_rfc3339();
        let run_id = format!("geolocation-vectorization:{}", geolocation.id);
        let vector_node_id = qdrant_vector_node_id(GEOLOCATION_COLLECTION, vector_id);
        let mut nodes = vec![
            json!({
                "label": "Geolocation",
                "id": geolocation.id,
            }),
            json!({
                "label": "GeolocationVectorizationRun",
                "id": run_id,
                "geolocation_id": geolocation.id,
                "model": model,
                "processed_at": processed_at,
                "embedding_len": embedding_len,
            }),
            qdrant_vector_node(
                GEOLOCATION_COLLECTION,
                vector_id,
                "geolocation",
                Some(model),
            ),
        ];
        let mut relationships = vec![
            json!({
                "from": geolocation.id,
                "to": run_id,
                "type": "HAS_GEOLOCATION_VECTORIZATION_RUN",
            }),
            json!({
                "from": run_id,
                "to": geolocation.id,
                "type": "PROCESSED_GEOLOCATION",
            }),
            json!({
                "from": geolocation.id,
                "to": vector_node_id,
                "type": "HAS_GEOLOCATION_VECTOR",
            }),
            json!({
                "from": vector_node_id,
                "to": geolocation.id,
                "type": "DERIVED_FROM",
            }),
            json!({
                "from": run_id,
                "to": vector_node_id,
                "type": "PRODUCED",
            }),
        ];

        if let Some(sensation_id) = &geolocation.sensation_id {
            nodes.push(json!({
                "label": "Sensation",
                "id": sensation_id,
            }));
            relationships.push(json!({
                "from": sensation_id,
                "to": run_id,
                "type": "PRODUCED",
            }));
            relationships.push(json!({
                "from": sensation_id,
                "to": vector_node_id,
                "type": "PRODUCED",
            }));
        }

        self.store_data(&json!({
            "op": "merge_graph",
            "nodes": nodes,
            "relationships": relationships,
        }))
        .await
    }

    /// Attach voice recognition results to an existing `AudioClip` graph node.
    pub async fn attach_voice_recognition(
        &self,
        clip: &GraphVoiceClip,
        model: &str,
        recognition: &GraphVoiceRecognition,
    ) -> Result<()> {
        let processed_at = chrono::Utc::now().to_rfc3339();
        let run_id = format!("voice-recognition:{}", clip.id);
        let signature_id = format!("voice-signature:{}", recognition.signature.user_id);
        let sample_id = recognition.sample.id.clone();
        let vector_id = qdrant_vector_node_id(VOICE_COLLECTION, &recognition.vector_id);
        let nodes = vec![
            json!({
                "label": "AudioClip",
                "id": clip.id,
            }),
            json!({
                "label": "VoiceRecognitionRun",
                "id": run_id,
                "audio_clip_id": clip.id,
                "model": model,
                "processed_at": processed_at,
                "embedding_len": recognition.embedding_len,
            }),
            voice_signature_node(&signature_id, &recognition.signature),
            voice_sample_node(&recognition.sample, &clip.id),
            qdrant_vector_node(
                VOICE_COLLECTION,
                &recognition.vector_id,
                "voice",
                Some(model),
            ),
        ];
        let mut relationships = vec![
            json!({
                "from": clip.id,
                "to": run_id,
                "type": "HAS_VOICE_RECOGNITION_RUN",
            }),
            json!({
                "from": run_id,
                "to": clip.id,
                "type": "PROCESSED_AUDIO",
            }),
            json!({
                "from": run_id,
                "to": signature_id,
                "type": "PRODUCED_SIGNATURE",
            }),
            json!({
                "from": run_id,
                "to": sample_id,
                "type": "PRODUCED_SAMPLE",
            }),
            json!({
                "from": signature_id,
                "to": sample_id,
                "type": "HAS_VOICE_SAMPLE",
            }),
            json!({
                "from": sample_id,
                "to": clip.id,
                "type": "DERIVED_FROM",
            }),
            json!({
                "from": signature_id,
                "to": vector_id,
                "type": "HAS_VOICE_VECTOR",
            }),
            json!({
                "from": sample_id,
                "to": vector_id,
                "type": "HAS_VOICE_VECTOR",
            }),
            json!({
                "from": run_id,
                "to": vector_id,
                "type": "PRODUCED",
            }),
        ];

        if let Some(sensation_id) = &clip.sensation_id {
            relationships.push(json!({
                "from": sensation_id,
                "to": run_id,
                "type": "PRODUCED",
            }));
            relationships.push(json!({
                "from": sensation_id,
                "to": signature_id,
                "type": "PRODUCED",
            }));
            relationships.push(json!({
                "from": sensation_id,
                "to": vector_id,
                "type": "PRODUCED",
            }));
        }

        self.store_data(&json!({
            "op": "merge_graph",
            "nodes": nodes,
            "relationships": relationships,
        }))
        .await
    }

    /// Store `data` in the graph database.
    pub async fn store_data(&self, data: &Value) -> Result<()> {
        let statements = graph_statements(data)?;
        if statements.is_empty() {
            return Ok(());
        }
        let endpoint = self.http_endpoint()?;
        let client = reqwest::Client::new();
        self.ensure_constraint(&client, &endpoint).await?;
        commit_neo4j_statements(
            &client,
            &endpoint,
            &self.user,
            &self.pass,
            &statements,
            "committing graph records",
        )
        .await?;
        info!(
            target: "neo4j",
            uri = %self.uri,
            endpoint = %endpoint,
            count = statements.len(),
            "stored graph data"
        );
        Ok(())
    }

    async fn ensure_constraint(&self, client: &reqwest::Client, endpoint: &Url) -> Result<()> {
        if self.constraint_ensured.load(Ordering::SeqCst) {
            return Ok(());
        }
        let statements = [CypherStatement {
            statement: "CREATE CONSTRAINT pete_graph_node_id IF NOT EXISTS FOR (n:GraphNode) REQUIRE n.id IS UNIQUE".into(),
            parameters: json!({}),
        }];
        commit_neo4j_statements(
            client,
            endpoint,
            &self.user,
            &self.pass,
            &statements,
            "ensuring graph node constraint",
        )
        .await?;
        self.constraint_ensured.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn http_endpoint(&self) -> Result<Url> {
        let parsed =
            Url::parse(&self.uri).with_context(|| format!("invalid Neo4j URI {}", self.uri))?;
        let mut url = match parsed.scheme() {
            "http" | "https" => parsed,
            "bolt" | "neo4j" => neo4j_http_url(&parsed, "http", 7474)?,
            "bolt+s" | "neo4j+s" => neo4j_http_url(&parsed, "https", 7473)?,
            scheme => bail!("unsupported Neo4j URI scheme {scheme}"),
        };
        url.set_path("/db/neo4j/tx/commit");
        url.set_query(None);
        url.set_fragment(None);
        Ok(url)
    }
}

fn graph_snapshot_from_row(row: &Value) -> Result<GraphSnapshot> {
    let values = row
        .as_array()
        .context("Neo4j graph snapshot row was not an array")?;
    let nodes = values
        .first()
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(graph_node_snapshot_from_value)
        .collect::<Result<Vec<_>>>()?;
    let relationships = values
        .get(1)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(graph_relationship_snapshot_from_value)
        .collect::<Result<Vec<_>>>()?;
    Ok(GraphSnapshot {
        nodes,
        relationships,
    })
}

fn graph_node_snapshot_from_value(value: Value) -> Result<GraphNodeSnapshot> {
    let object = value
        .as_object()
        .context("Neo4j graph node snapshot was not an object")?;
    let id = object
        .get("id")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .context("Neo4j graph node snapshot is missing id")?;
    let labels = object
        .get("labels")
        .and_then(Value::as_array)
        .map(|labels| {
            labels
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let properties = sanitize_graph_properties(
        object
            .get("properties")
            .cloned()
            .unwrap_or_else(|| json!({})),
    );
    Ok(GraphNodeSnapshot {
        id,
        labels,
        properties,
    })
}

fn graph_relationship_snapshot_from_value(value: Value) -> Result<GraphRelationshipSnapshot> {
    let object = value
        .as_object()
        .context("Neo4j graph relationship snapshot was not an object")?;
    Ok(GraphRelationshipSnapshot {
        id: snapshot_string(object, "id")?,
        source: snapshot_string(object, "source")?,
        target: snapshot_string(object, "target")?,
        relationship_type: snapshot_string(object, "type")?,
        properties: sanitize_graph_properties(
            object
                .get("properties")
                .cloned()
                .unwrap_or_else(|| json!({})),
        ),
    })
}

fn graph_node_details_from_row(row: &Value) -> Result<GraphNodeDetails> {
    let values = row
        .as_array()
        .context("Neo4j graph node details row was not an array")?;
    let node_value = values
        .first()
        .cloned()
        .context("Neo4j graph node details row is missing node")?;
    let node = graph_node_details_from_value(node_value)?;
    let relationships = values
        .get(1)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(graph_relationship_snapshot_from_value)
        .collect::<Result<Vec<_>>>()?;
    Ok(GraphNodeDetails {
        relationships,
        ..node
    })
}

fn graph_node_details_from_value(value: Value) -> Result<GraphNodeDetails> {
    let object = value
        .as_object()
        .context("Neo4j graph node details was not an object")?;
    let id = object
        .get("id")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .context("Neo4j graph node details is missing id")?;
    let labels = object
        .get("labels")
        .and_then(Value::as_array)
        .map(|labels| {
            labels
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let properties = sanitize_graph_detail_properties(
        object
            .get("properties")
            .cloned()
            .unwrap_or_else(|| json!({})),
    );
    Ok(GraphNodeDetails {
        id,
        labels,
        properties,
        relationships: Vec::new(),
    })
}

fn snapshot_string(object: &Map<String, Value>, key: &str) -> Result<String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .with_context(|| format!("Neo4j graph snapshot is missing {key}"))
}

fn sanitize_graph_properties(value: Value) -> Value {
    let Value::Object(object) = value else {
        return value;
    };
    let mut sanitized = Map::new();
    for (key, value) in object {
        if should_omit_graph_snapshot_property(&key) {
            continue;
        }
        sanitized.insert(key, value);
    }
    Value::Object(sanitized)
}

fn should_omit_graph_snapshot_property(key: &str) -> bool {
    matches!(key, "base64" | "crop_base64" | "embedding" | "raw_json")
}

fn sanitize_graph_detail_properties(value: Value) -> Value {
    let Value::Object(object) = value else {
        return value;
    };
    let mut sanitized = Map::new();
    for (key, value) in object {
        if should_omit_graph_detail_property(&key) {
            continue;
        }
        sanitized.insert(key, value);
    }
    Value::Object(sanitized)
}

fn should_omit_graph_detail_property(key: &str) -> bool {
    matches!(key, "embedding" | "raw_json")
}

fn graph_audio_clip_from_row(row: &Value) -> Result<GraphAudioClip> {
    let values = row
        .as_array()
        .context("Neo4j audio clip row was not an array")?;
    let id = row_string(values, 0, "id")?;
    let clip = AudioClip {
        mime: row_string(values, 1, "mime")?,
        base64: row_string(values, 2, "base64")?,
        sample_rate: row_u32(values, 3, "sample_rate")?,
        channels: row_u16(values, 4, "channels")?,
        transcript: None,
        captured_at: row_optional_string(values, 5),
    };
    Ok(GraphAudioClip {
        id,
        clip,
        occurred_at: row_optional_string(values, 6),
        sensation_id: row_optional_string(values, 7),
    })
}

fn graph_audio_clip_window_from_rows(rows: &[Value]) -> Result<Option<GraphAudioClipWindow>> {
    let Some(first) = rows.first() else {
        return Ok(None);
    };
    let first_values = first
        .as_array()
        .context("Neo4j audio clip window row was not an array")?;
    let anchor_id = row_string(first_values, 0, "anchor_id")?;
    let clips = rows
        .iter()
        .map(graph_audio_clip_from_window_row)
        .collect::<Result<Vec<_>>>()?;
    Ok(Some(GraphAudioClipWindow { anchor_id, clips }))
}

fn graph_audio_clip_from_window_row(row: &Value) -> Result<GraphAudioClip> {
    let values = row
        .as_array()
        .context("Neo4j audio clip window row was not an array")?;
    let id = row_string(values, 1, "id")?;
    let clip = AudioClip {
        mime: row_string(values, 2, "mime")?,
        base64: row_string(values, 3, "base64")?,
        sample_rate: row_u32(values, 4, "sample_rate")?,
        channels: row_u16(values, 5, "channels")?,
        transcript: None,
        captured_at: row_optional_string(values, 6),
    };
    Ok(GraphAudioClip {
        id,
        clip,
        occurred_at: row_optional_string(values, 7),
        sensation_id: row_optional_string(values, 8),
    })
}

fn graph_consolidated_speech_candidate_from_row(
    row: &Value,
) -> Result<GraphConsolidatedSpeechCandidate> {
    let values = row
        .as_array()
        .context("Neo4j speech consolidation row was not an array")?;
    let sources = values
        .get(4)
        .and_then(Value::as_array)
        .context("Neo4j speech consolidation row is missing sources")?
        .iter()
        .map(graph_consolidated_speech_source_from_value)
        .collect::<Result<Vec<_>>>()?;
    Ok(GraphConsolidatedSpeechCandidate {
        transcription_id: row_string(values, 0, "transcription_id")?,
        transcript: row_string(values, 1, "transcript")?,
        source_started_at: row_optional_string(values, 2),
        source_ended_at: row_optional_string(values, 3),
        sources,
    })
}

fn graph_consolidated_speech_source_from_value(
    value: &Value,
) -> Result<GraphConsolidatedSpeechSource> {
    let object = value
        .as_object()
        .context("Neo4j speech consolidation source was not an object")?;
    let id = object_string(object, "id")?;
    let clip = GraphAudioClip {
        id,
        clip: AudioClip {
            mime: object_string(object, "mime")?,
            base64: object_string(object, "base64")?,
            sample_rate: object_u32(object, "sample_rate")?,
            channels: object_u16(object, "channels")?,
            transcript: object_optional_string(object, "transcript"),
            captured_at: object_optional_string(object, "captured_at"),
        },
        occurred_at: object_optional_string(object, "occurred_at"),
        sensation_id: object_optional_string(object, "sensation_id"),
    };
    Ok(GraphConsolidatedSpeechSource {
        index: object_usize(object, "index")?,
        clip,
        start_ms: object_u32(object, "start_ms")?,
        end_ms: object_u32(object, "end_ms")?,
        transcription_ids: object_string_vec(object, "transcription_ids"),
    })
}

fn graph_timeline_window_from_rows(rows: &[Value]) -> Result<Option<GraphTimelineWindow>> {
    let Some(first) = rows.first() else {
        return Ok(None);
    };
    let first_values = first
        .as_array()
        .context("Neo4j timeline window row was not an array")?;
    let anchor_id = row_string(first_values, 0, "anchor_id")?;
    let anchor_at = row_string(first_values, 1, "anchor_at")?;
    let items = rows
        .iter()
        .map(graph_timeline_item_from_row)
        .collect::<Result<Vec<_>>>()?;
    Ok(Some(GraphTimelineWindow {
        anchor_id,
        anchor_at,
        items,
    }))
}

fn graph_timeline_item_from_row(row: &Value) -> Result<GraphTimelineItem> {
    let values = row
        .as_array()
        .context("Neo4j timeline item row was not an array")?;
    Ok(GraphTimelineItem {
        id: row_string(values, 2, "timeline item id")?,
        event_id: row_string(values, 3, "timeline item event id")?,
        labels: row_string_vec(values, 4),
        text: row_string(values, 5, "timeline item text")?,
        occurred_at: row_string(values, 6, "timeline item timestamp")?,
    })
}

fn graph_cluster_item_from_row(row: &Value) -> Result<GraphClusterItem> {
    let values = row
        .as_array()
        .context("Neo4j vector cluster item row was not an array")?;
    Ok(GraphClusterItem {
        vector_id: row_string(values, 0, "cluster item vector id")?,
        node_id: row_string(values, 1, "cluster item node id")?,
        labels: row_string_vec(values, 2),
        text: row_string(values, 3, "cluster item text")?,
        stimuli: row_string_vec(values, 4),
        edges: row_string_vec(values, 5),
        neighbors: row_string_vec(values, 6),
    })
}

fn graph_speech_segment_audio_from_row(row: &Value) -> Result<GraphSpeechSegmentAudio> {
    let values = row
        .as_array()
        .context("Neo4j speech segment audio row was not an array")?;
    Ok(GraphSpeechSegmentAudio {
        segment_id: row_string(values, 0, "segment_id")?,
        text: row_string(values, 1, "text")?,
        audio_clip_id: row_string(values, 2, "audio_clip_id")?,
        mime: row_string(values, 3, "mime")?,
        base64: row_string(values, 4, "base64")?,
        sample_rate: row_u32(values, 5, "sample_rate")?,
        channels: row_u16(values, 6, "channels")?,
        start_ms: row_u32(values, 7, "start_ms")?,
        end_ms: row_u32(values, 8, "end_ms")?,
    })
}

fn graph_voice_clip_from_row(row: &Value) -> Result<GraphVoiceClip> {
    let values = row
        .as_array()
        .context("Neo4j voice clip row was not an array")?;
    let id = row_string(values, 0, "id")?;
    let clip = AudioClip {
        mime: row_string(values, 1, "mime")?,
        base64: row_string(values, 2, "base64")?,
        sample_rate: row_u32(values, 3, "sample_rate")?,
        channels: row_u16(values, 4, "channels")?,
        transcript: None,
        captured_at: row_optional_string(values, 5),
    };
    Ok(GraphVoiceClip {
        id,
        clip,
        occurred_at: row_optional_string(values, 6),
        sensation_id: row_optional_string(values, 7),
    })
}

fn graph_image_frame_from_row(row: &Value) -> Result<GraphImageFrame> {
    let values = row
        .as_array()
        .context("Neo4j image frame row was not an array")?;
    let id = row_string(values, 0, "id")?;
    let image = ImageData {
        mime: row_string(values, 1, "mime")?,
        base64: row_string(values, 2, "base64")?,
        captured_at: row_optional_string(values, 3),
    };
    Ok(GraphImageFrame {
        id,
        image,
        occurred_at: row_optional_string(values, 4),
        sensation_id: row_optional_string(values, 5),
    })
}

fn graph_geolocation_from_row(row: &Value) -> Result<GraphGeolocation> {
    let values = row
        .as_array()
        .context("Neo4j geolocation row was not an array")?;
    let id = row_string(values, 0, "id")?;
    Ok(GraphGeolocation {
        id,
        loc: GeoLoc {
            latitude: row_f64(values, 1, "latitude")?,
            longitude: row_f64(values, 2, "longitude")?,
            observed_at: row_optional_string(values, 3),
        },
        occurred_at: row_optional_string(values, 4),
        sensation_id: row_optional_string(values, 5),
    })
}

fn row_string(values: &[Value], index: usize, name: &str) -> Result<String> {
    values
        .get(index)
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .with_context(|| format!("Neo4j audio clip row is missing {name}"))
}

fn row_optional_string(values: &[Value], index: usize) -> Option<String> {
    values
        .get(index)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn row_string_vec(values: &[Value], index: usize) -> Vec<String> {
    values
        .get(index)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn object_string(object: &Map<String, Value>, key: &str) -> Result<String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .with_context(|| format!("Neo4j object is missing {key}"))
}

fn object_optional_string(object: &Map<String, Value>, key: &str) -> Option<String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn object_string_vec(object: &Map<String, Value>, key: &str) -> Vec<String> {
    object
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn object_u32(object: &Map<String, Value>, key: &str) -> Result<u32> {
    let value = object
        .get(key)
        .and_then(Value::as_u64)
        .with_context(|| format!("Neo4j object is missing numeric {key}"))?;
    u32::try_from(value).with_context(|| format!("Neo4j object {key} is out of range"))
}

fn object_u16(object: &Map<String, Value>, key: &str) -> Result<u16> {
    let value = object_u32(object, key)?;
    u16::try_from(value).with_context(|| format!("Neo4j object {key} is out of range"))
}

fn object_usize(object: &Map<String, Value>, key: &str) -> Result<usize> {
    let value = object
        .get(key)
        .and_then(Value::as_u64)
        .with_context(|| format!("Neo4j object is missing numeric {key}"))?;
    usize::try_from(value).with_context(|| format!("Neo4j object {key} is out of range"))
}

fn row_u32(values: &[Value], index: usize, name: &str) -> Result<u32> {
    let value = values
        .get(index)
        .and_then(Value::as_u64)
        .with_context(|| format!("Neo4j audio clip row is missing numeric {name}"))?;
    u32::try_from(value).with_context(|| format!("Neo4j audio clip {name} is out of range"))
}

fn row_u16(values: &[Value], index: usize, name: &str) -> Result<u16> {
    let value = row_u32(values, index, name)?;
    u16::try_from(value).with_context(|| format!("Neo4j audio clip {name} is out of range"))
}

fn row_f64(values: &[Value], index: usize, name: &str) -> Result<f64> {
    values
        .get(index)
        .and_then(Value::as_f64)
        .with_context(|| format!("Neo4j row is missing numeric {name}"))
}

async fn commit_neo4j_statements(
    client: &reqwest::Client,
    endpoint: &Url,
    user: &str,
    pass: &str,
    statements: &[CypherStatement],
    action: &str,
) -> Result<()> {
    let body = json!({
        "statements": statements.iter().map(|statement| {
            json!({
                "statement": statement.statement,
                "parameters": statement.parameters,
            })
        }).collect::<Vec<_>>()
    });
    let response = client
        .post(endpoint.clone())
        .basic_auth(user, Some(pass))
        .json(&body)
        .timeout(NEO4J_REQUEST_TIMEOUT)
        .send()
        .await
        .with_context(|| format!("failed while {action} at {endpoint}"))?;
    if !response.status().is_success() {
        return Err(unexpected_neo4j_response(response, action).await);
    }
    let body: Value = response
        .json()
        .await
        .with_context(|| format!("failed to decode Neo4j response while {action}"))?;
    if let Some(errors) = body.get("errors").and_then(Value::as_array) {
        if !errors.is_empty() {
            bail!("Neo4j returned errors while {action}: {errors:?}");
        }
    }
    Ok(())
}

async fn query_neo4j_rows(
    client: &reqwest::Client,
    endpoint: &Url,
    user: &str,
    pass: &str,
    statement: CypherStatement,
    action: &str,
) -> Result<Vec<Value>> {
    let body = json!({
        "statements": [{
            "statement": statement.statement,
            "parameters": statement.parameters,
            "resultDataContents": ["row"],
        }]
    });
    let response = client
        .post(endpoint.clone())
        .basic_auth(user, Some(pass))
        .json(&body)
        .timeout(NEO4J_REQUEST_TIMEOUT)
        .send()
        .await
        .with_context(|| format!("failed while {action} at {endpoint}"))?;
    if !response.status().is_success() {
        return Err(unexpected_neo4j_response(response, action).await);
    }
    let body: Value = response
        .json()
        .await
        .with_context(|| format!("failed to decode Neo4j response while {action}"))?;
    if let Some(errors) = body.get("errors").and_then(Value::as_array) {
        if !errors.is_empty() {
            bail!("Neo4j returned errors while {action}: {errors:?}");
        }
    }
    let data = body
        .pointer("/results/0/data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    Ok(data
        .into_iter()
        .filter_map(|entry| entry.get("row").cloned())
        .collect())
}

fn neo4j_http_url(source: &Url, scheme: &str, default_port: u16) -> Result<Url> {
    let host = source
        .host_str()
        .with_context(|| format!("Neo4j URI {} is missing a host", source.as_str()))?;
    let port = match source.port() {
        Some(7687) | None => default_port,
        Some(port) => port,
    };
    Url::parse(&format!("{scheme}://{host}:{port}"))
        .with_context(|| format!("failed to convert {} to {scheme}", source.as_str()))
}

async fn unexpected_neo4j_response(response: reqwest::Response, action: &str) -> anyhow::Error {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    anyhow!("Neo4j returned {status} while {action}: {body}")
}

#[derive(Debug, Clone)]
struct CypherStatement {
    statement: String,
    parameters: Value,
}

fn graph_statements(data: &Value) -> Result<Vec<CypherStatement>> {
    let mut statements = Vec::new();

    if data.get("op").and_then(Value::as_str) == Some("merge_graph") {
        let nodes = data
            .get("nodes")
            .and_then(Value::as_array)
            .context("merge_graph record is missing nodes array")?;
        for node in nodes {
            statements.push(node_statement(node)?);
        }
        let relationships = data
            .get("relationships")
            .and_then(Value::as_array)
            .map(|relationships| {
                relationships
                    .iter()
                    .map(relationship_statement)
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?
            .unwrap_or_default();
        statements.extend(relationships);
    } else {
        statements.push(raw_payload_statement(data)?);
    }

    Ok(statements)
}

fn node_statement(node: &Value) -> Result<CypherStatement> {
    let label = node
        .get("label")
        .and_then(Value::as_str)
        .context("graph node is missing label")?;
    validate_graph_name(label, "label")?;
    let id = node
        .get("id")
        .and_then(Value::as_str)
        .context("graph node is missing id")?;
    let props = property_map(node);
    Ok(CypherStatement {
        statement: format!("MERGE (n:GraphNode {{id: $id}}) SET n += $props SET n:`{label}`"),
        parameters: json!({
            "id": id,
            "props": props,
        }),
    })
}

fn relationship_statement(rel: &Value) -> Result<CypherStatement> {
    let rel_type = rel
        .get("type")
        .and_then(Value::as_str)
        .context("graph relationship is missing type")?;
    validate_graph_name(rel_type, "relationship type")?;
    let from = rel
        .get("from")
        .and_then(Value::as_str)
        .context("graph relationship is missing from")?;
    let to = rel
        .get("to")
        .and_then(Value::as_str)
        .context("graph relationship is missing to")?;
    let props = property_map(rel);
    Ok(CypherStatement {
        statement: format!(
            "MATCH (from:GraphNode {{id: $from}}), (to:GraphNode {{id: $to}}) MERGE (from)-[r:`{rel_type}`]->(to) SET r += $props"
        ),
        parameters: json!({
            "from": from,
            "to": to,
            "props": props,
        }),
    })
}

fn raw_payload_statement(data: &Value) -> Result<CypherStatement> {
    let raw_json = serde_json::to_string(data)?;
    let id = stable_json_id("raw-payload", data);
    Ok(CypherStatement {
        statement: "MERGE (n:GraphNode {id: $id}) SET n += $props SET n:`RawPayload`".into(),
        parameters: json!({
            "id": id,
            "props": {
                "id": id,
                "raw_json": raw_json,
            }
        }),
    })
}

fn property_map(value: &Value) -> Value {
    let Some(object) = value.as_object() else {
        return json!({});
    };
    let mut props = Map::new();
    for (key, value) in object {
        if matches!(
            key.as_str(),
            "label" | "merge_key" | "from" | "to" | "type" | "relationships" | "nodes" | "op"
        ) {
            continue;
        }
        if let Some(prop) = graph_property(value) {
            props.insert(key.clone(), prop);
        }
    }
    Value::Object(props)
}

fn graph_property(value: &Value) -> Option<Value> {
    match value {
        Value::Null | Value::Object(_) => None,
        Value::Array(items) => {
            let props = items.iter().filter_map(graph_property).collect::<Vec<_>>();
            Some(Value::Array(props))
        }
        Value::String(_) | Value::Bool(_) | Value::Number(_) => Some(value.clone()),
    }
}

fn validate_graph_name(name: &str, kind: &str) -> Result<()> {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        bail!("empty Neo4j {kind}");
    };
    if !(first == '_' || first.is_ascii_alphabetic()) {
        bail!("invalid Neo4j {kind}: {name}");
    }
    if chars.any(|c| !(c == '_' || c.is_ascii_alphanumeric())) {
        bail!("invalid Neo4j {kind}: {name}");
    }
    Ok(())
}

struct GraphStimulusTarget {
    stimulus_id: String,
    target_id: String,
    nodes: Vec<Value>,
    relationships: Vec<Value>,
}

struct GraphVectorLink<'a> {
    owner_id: String,
    relationship: &'a str,
    collection: &'a str,
    point_id: String,
    kind: &'a str,
    model: Option<&'a str>,
}

fn impression_graph_record(
    impression: &Impression<Value>,
    stimulus_targets: &[GraphStimulusTarget],
    vector_links: &[GraphVectorLink<'_>],
) -> Result<Value> {
    let impression_id = impression_id(impression)?;
    let mut nodes = vec![json!({
        "label": "Impression",
        "id": impression_id,
        "summary": impression.summary,
        "emoji": impression.emoji,
        "timestamp": impression.timestamp.to_rfc3339(),
    })];
    let mut relationships = Vec::new();

    for target in stimulus_targets {
        nodes.extend(target.nodes.clone());
        relationships.extend(target.relationships.clone());
        relationships.push(json!({
            "from": impression_id,
            "to": target.stimulus_id,
            "type": "HAS_STIMULUS",
        }));
        relationships.push(json!({
            "from": impression_id,
            "to": target.target_id,
            "type": "INTERPRETS",
        }));
    }

    for link in vector_links {
        let vector_id = qdrant_vector_node_id(link.collection, &link.point_id);
        nodes.push(qdrant_vector_node(
            link.collection,
            &link.point_id,
            link.kind,
            link.model,
        ));
        relationships.push(json!({
            "from": link.owner_id,
            "to": vector_id,
            "type": link.relationship,
        }));
    }

    Ok(json!({
        "op": "merge_graph",
        "nodes": nodes,
        "relationships": relationships,
    }))
}

fn stimulus_target(stimulus: &Stimulus<Value>) -> Result<GraphStimulusTarget> {
    let stimulus_id = stable_json_id(
        "stimulus",
        &json!({
            "timestamp": stimulus.timestamp.to_rfc3339(),
            "what": stimulus.what,
        }),
    );
    let raw_json = serde_json::to_string(&stored_payload_json(&stimulus.what))?;
    let mut nodes = vec![json!({
        "label": "Stimulus",
        "id": stimulus_id,
        "timestamp": stimulus.timestamp.to_rfc3339(),
        "raw_json": raw_json,
    })];

    let (target_id, target_node) = payload_target_node(&stimulus.what, stimulus.timestamp)?;
    nodes.push(target_node);
    let relationships = vec![json!({
        "from": stimulus_id,
        "to": target_id,
        "type": "REFERS_TO",
    })];

    Ok(GraphStimulusTarget {
        stimulus_id,
        target_id,
        nodes,
        relationships,
    })
}

fn payload_target_node(
    value: &Value,
    occurred_at: chrono::DateTime<chrono::Utc>,
) -> Result<(String, Value)> {
    if let Ok(image) = serde_json::from_value::<ImageData>(value.clone()) {
        let id = image_content_id(&image);
        return Ok((
            id.clone(),
            image_node(&image, &id, occurred_at.to_rfc3339()),
        ));
    }
    if let Ok(loc) = serde_json::from_value::<GeoLoc>(value.clone()) {
        let id = geoloc_content_id(&loc);
        return Ok((
            id.clone(),
            geolocation_node(&loc, &id, occurred_at.to_rfc3339()),
        ));
    }
    if let Ok(audio) = serde_json::from_value::<AudioClip>(value.clone()) {
        let id = audio_clip_id(&audio);
        return Ok((
            id.clone(),
            audio_node(&audio, &id, occurred_at.to_rfc3339()),
        ));
    }
    if let Ok(heartbeat) = serde_json::from_value::<Heartbeat>(value.clone()) {
        let id = format!("heartbeat:{}", heartbeat.timestamp.to_rfc3339());
        return Ok((
            id.clone(),
            heartbeat_node(&heartbeat, &id, occurred_at.to_rfc3339()),
        ));
    }
    if let Ok(object) = serde_json::from_value::<ObjectInfo>(value.clone()) {
        let id = object_info_id(&object, occurred_at.to_rfc3339());
        return Ok((
            id.clone(),
            object_info_node(&object, &id, occurred_at.to_rfc3339()),
        ));
    }
    if let Some(text) = value.as_str() {
        let node = json!({
            "label": "TextObservation",
            "id": stable_string_id("text", text),
            "text": text,
            "occurred_at": occurred_at.to_rfc3339(),
        });
        return Ok((node["id"].as_str().unwrap().to_string(), node));
    }

    let id = stable_json_id("payload", value);
    Ok((
        id.clone(),
        json!({
            "label": "RawPayload",
            "id": id,
            "raw_json": serde_json::to_string(&stored_payload_json(value))?,
            "occurred_at": occurred_at.to_rfc3339(),
        }),
    ))
}

fn impression_id(impression: &Impression<Value>) -> Result<String> {
    Ok(stable_json_id(
        "impression",
        &json!({
            "summary": impression.summary,
            "emoji": impression.emoji,
            "timestamp": impression.timestamp.to_rfc3339(),
            "stimuli": impression.stimuli.iter().map(|stimulus| {
                json!({
                    "timestamp": stimulus.timestamp.to_rfc3339(),
                    "what": stimulus.what,
                })
            }).collect::<Vec<_>>(),
        }),
    ))
}

pub(crate) fn qdrant_vector_node_id(collection: &str, point_id: &str) -> String {
    format!("qdrant:{collection}:{point_id}")
}

pub(crate) fn qdrant_vector_node(
    collection: &str,
    point_id: &str,
    kind: &str,
    model: Option<&str>,
) -> Value {
    json!({
        "label": "Vector",
        "id": qdrant_vector_node_id(collection, point_id),
        "database": "qdrant",
        "collection": collection,
        "point_id": point_id,
        "kind": kind,
        "model": model,
    })
}

fn image_node(image: &ImageData, id: &str, occurred_at: String) -> Value {
    json!({
        "label": "Image",
        "id": id,
        "mime": image.mime.clone(),
        "base64": image.base64.clone(),
        "captured_at": image.captured_at.clone(),
        "occurred_at": occurred_at,
    })
}

fn audio_node(audio: &AudioClip, id: &str, occurred_at: String) -> Value {
    json!({
        "label": "AudioClip",
        "id": id,
        "mime": audio.mime.clone(),
        "base64": audio.base64.clone(),
        "sample_rate": audio.sample_rate,
        "channels": audio.channels,
        "transcript": audio.transcript.clone(),
        "captured_at": audio.captured_at.clone(),
        "occurred_at": occurred_at,
    })
}

fn voice_signature_node(id: &str, signature: &GraphVoiceSignature) -> Value {
    json!({
        "label": "VoiceSignature",
        "id": id,
        "user_id": signature.user_id,
        "fundamental_frequency": signature.fundamental_frequency,
        "frequency_range_min": signature.frequency_range.0,
        "frequency_range_max": signature.frequency_range.1,
        "formant_frequencies": signature.formant_frequencies,
        "speech_rate": signature.speech_rate,
        "mfcc_signature": signature.mfcc_signature,
        "spectral_centroid": signature.spectral_centroid,
        "jitter": signature.jitter,
        "shimmer": signature.shimmer,
        "harmonic_to_noise_ratio": signature.harmonic_to_noise_ratio,
        "sample_count": signature.sample_count,
        "last_updated": signature.last_updated.to_rfc3339(),
        "tags": signature.tags,
        "immutable": false,
    })
}

fn voice_sample_node(sample: &GraphVoiceSample, clip_id: &str) -> Value {
    json!({
        "label": "VoiceSample",
        "id": sample.id,
        "user_id": sample.user_id,
        "audio_clip_id": clip_id,
        "duration_ms": sample.duration_ms,
        "sample_rate": sample.sample_rate,
        "fundamental_frequency": sample.fundamental_frequency,
        "formant_frequencies": sample.formant_frequencies,
        "mfcc": sample.mfcc,
        "quality_score": sample.quality_score,
        "timestamp": sample.timestamp.to_rfc3339(),
    })
}

fn geolocation_node(loc: &GeoLoc, id: &str, occurred_at: String) -> Value {
    json!({
        "label": "Geolocation",
        "id": id,
        "latitude": loc.latitude,
        "longitude": loc.longitude,
        "observed_at": loc.observed_at.clone(),
        "occurred_at": occurred_at,
    })
}

fn heartbeat_node(heartbeat: &Heartbeat, id: &str, occurred_at: String) -> Value {
    json!({
        "label": "Heartbeat",
        "id": id,
        "timestamp": heartbeat.timestamp.to_rfc3339(),
        "occurred_at": occurred_at,
    })
}

fn object_info_node(object: &ObjectInfo, id: &str, occurred_at: String) -> Value {
    json!({
        "label": "ObjectObservation",
        "id": id,
        "object_label": object.label.clone(),
        "embedding_len": object.embedding.len(),
        "occurred_at": occurred_at,
    })
}

fn object_info_id(object: &ObjectInfo, occurred_at: String) -> String {
    format!(
        "object:{}:{}:{}",
        object.label.clone().unwrap_or_else(|| "unknown".into()),
        object.embedding.len(),
        occurred_at
    )
}

fn stored_payload_json(value: &Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(key, value)| {
                    let stored = if key == "embedding" {
                        json!({
                            "omitted": true,
                            "len": value.as_array().map_or(0, Vec::len),
                        })
                    } else {
                        stored_payload_json(value)
                    };
                    (key.clone(), stored)
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(stored_payload_json).collect()),
        _ => value.clone(),
    }
}

fn stable_json_id(prefix: &str, value: &Value) -> String {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    stable_bytes_id(prefix, &bytes)
}

fn stable_string_id(prefix: &str, value: &str) -> String {
    stable_bytes_id(prefix, value.as_bytes())
}

fn stable_bytes_id(prefix: &str, bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{prefix}:sha256:{:x}", hasher.finalize())
}

fn spans_overlap(a_start_ms: u32, a_end_ms: u32, b_start_ms: u32, b_end_ms: u32) -> bool {
    a_start_ms < b_end_ms && b_start_ms < a_end_ms
}

fn clip_local_overlap(
    segment_start_ms: u32,
    segment_end_ms: u32,
    source_start_ms: u32,
    source_end_ms: u32,
) -> (u32, u32) {
    let overlap_start = segment_start_ms.max(source_start_ms);
    let overlap_end = segment_end_ms.min(source_end_ms);
    (
        overlap_start.saturating_sub(source_start_ms),
        overlap_end.saturating_sub(source_start_ms),
    )
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
        let stimulus_targets = impression
            .stimuli
            .iter()
            .map(stimulus_target)
            .collect::<Result<Vec<_>>>()?;
        let impression_node_id = impression_id(impression)?;
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
        let mut vector_links = Vec::new();
        if let Some(v) = vector {
            for image_id in impression
                .stimuli
                .iter()
                .filter_map(|stim| serde_json::from_value::<ImageData>(stim.what.clone()).ok())
                .map(|image| image_content_id(&image))
            {
                match self
                    .qdrant
                    .store_image_description_vector_for_node(
                        &image_id,
                        &impression.summary,
                        &image_id,
                        &[&impression_node_id],
                        &v,
                    )
                    .await
                {
                    Ok(id) => {
                        let point_id = id.to_string();
                        vector_links.push(GraphVectorLink {
                            owner_id: image_id,
                            relationship: "HAS_IMAGE_DESCRIPTION_VECTOR",
                            collection: IMAGE_DESCRIPTION_COLLECTION,
                            point_id: point_id.clone(),
                            kind: "image_description",
                            model: None,
                        });
                        vector_links.push(GraphVectorLink {
                            owner_id: impression_node_id.clone(),
                            relationship: "HAS_IMAGE_DESCRIPTION_VECTOR",
                            collection: IMAGE_DESCRIPTION_COLLECTION,
                            point_id,
                            kind: "image_description",
                            model: None,
                        });
                    }
                    Err(e) => tracing::error!(?e, "failed to store image description vector"),
                }
            }
            match self
                .qdrant
                .store_vector_for_node(&impression.summary, Some(&impression_node_id), &v)
                .await
            {
                Ok(id) => vector_links.push(GraphVectorLink {
                    owner_id: impression_node_id.clone(),
                    relationship: "HAS_MEMORY_VECTOR",
                    collection: MEMORY_COLLECTION,
                    point_id: id.to_string(),
                    kind: "memory",
                    model: None,
                }),
                Err(e) => tracing::error!(?e, "failed to store vector"),
            }
        }
        let graph = impression_graph_record(impression, &stimulus_targets, &vector_links)?;
        self.neo4j.store_data(&graph).await?;
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
