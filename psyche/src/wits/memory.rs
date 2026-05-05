use crate::{
    AudioClip, GeoLoc, Heartbeat, ImageData, Impression, ObjectInfo, Stimulus, audio_clip_id,
    geoloc_content_id, image_content_id,
};
use anyhow::{Context, Result, anyhow, bail};
use async_trait::async_trait;
use lingproc::Vectorizer;
use reqwest::{StatusCode, Url};
use serde::Serialize;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
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
const FACE_COLLECTION: &str = "faces";
const GEOLOCATION_COLLECTION: &str = "geolocations";
const VOICE_COLLECTION: &str = "voices";
const QDRANT_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const NEO4J_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

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
        let id = self
            .upsert_vector(
                IMAGE_DESCRIPTION_COLLECTION,
                vector,
                json!({
                    "kind": "image_description",
                    "image_id": image_id,
                    "neo4j_node_id": neo4j_node_id,
                    "related_neo4j_node_ids": related_neo4j_node_ids,
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
        let id = self
            .upsert_vector(
                FACE_COLLECTION,
                vector,
                json!({
                    "kind": "face",
                    "face_id": face_id,
                    "neo4j_node_id": face_id,
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
                    "neo4j_node_id": clip_id,
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

impl Neo4jClient {
    pub fn new(uri: String, user: String, pass: String) -> Self {
        Self {
            uri,
            user,
            pass,
            constraint_ensured: Arc::new(AtomicBool::new(false)),
        }
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
        "captured_at": audio.captured_at.clone(),
        "occurred_at": occurred_at,
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
