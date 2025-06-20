use crate::Impression;
use crate::ling::Vectorizer;
use anyhow::Result;
use async_trait::async_trait;
use serde::Serialize;
use serde_json::Value;
use std::sync::Arc;
use tracing::info;

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
        let raw = serde_json::to_value(&impression.raw_data)?;
        let erased = Impression {
            id: impression.id,
            timestamp: impression.timestamp,
            headline: impression.headline.clone(),
            details: impression.details.clone(),
            raw_data: raw,
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
            let raw = serde_json::to_value(&imp.raw_data)?;
            erased.push(Impression {
                id: imp.id,
                timestamp: imp.timestamp,
                headline: imp.headline.clone(),
                details: imp.details.clone(),
                raw_data: raw,
            });
        }
        self.store_all(&erased).await
    }
}

/// Client for storing vectors in Qdrant.
#[derive(Clone, Default)]
pub struct QdrantClient;

impl QdrantClient {
    /// Store `vector` associated with `headline`.
    pub async fn store_vector(&self, headline: &str, vector: &[f32]) -> Result<()> {
        info!(target: "qdrant", ?headline, len = vector.len(), "stored vector");
        Ok(())
    }
}

/// Client for persisting raw data in Neo4j.
#[derive(Clone, Default)]
pub struct Neo4jClient;

impl Neo4jClient {
    /// Store `data` in the graph database.
    pub async fn store_data(&self, data: &Value) -> Result<()> {
        let json = serde_json::to_string(data)?;
        info!(target: "neo4j", %json, "stored data");
        Ok(())
    }
}

#[async_trait]
pub trait GraphStore: Send + Sync {
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
        let vector = self.vectorizer.vectorize(&impression.headline).await?;
        self.qdrant
            .store_vector(&impression.headline, &vector)
            .await?;
        self.neo4j.store_data(&impression.raw_data).await?;
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
