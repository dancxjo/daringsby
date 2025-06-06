use async_trait::async_trait;
use neo4rs::{query, Graph};
use qdrant_client::{
    qdrant::{PointStruct, UpsertPointsBuilder},
    Payload, Qdrant,
};

use crate::{Experience, MemoryError};

/// Stores experiences in both a vector database and a graph database.
pub struct GraphRag {
    vector: Qdrant,
    graph: Graph,
    collection: String,
}

impl GraphRag {
    /// Connect to Qdrant and Neo4j using the provided credentials.
    pub fn new(
        vector_url: &str,
        graph_uri: &str,
        user: &str,
        pass: &str,
    ) -> Result<Self, MemoryError> {
        let vector = Qdrant::from_url(vector_url).build()?;
        let graph = Graph::new(graph_uri, user, pass)?;
        Ok(Self {
            vector,
            graph,
            collection: "experiences".into(),
        })
    }
}

#[async_trait]
/// Abstract storage for [`Experience`] records.
#[async_trait]
pub trait Memory {
    /// Persist a single experience.
    async fn store(&self, exp: Experience) -> Result<(), MemoryError>;
}

#[async_trait]
impl Memory for GraphRag {
    async fn store(&self, exp: Experience) -> Result<(), MemoryError> {
        let point = PointStruct::new(
            exp.id.as_u128() as u64,
            exp.embedding.clone(),
            Payload::new(),
        );
        self.vector
            .upsert_points(UpsertPointsBuilder::new(&self.collection, vec![point]).wait(true))
            .await?;

        let q = query("MERGE (e:Experience {id: $id, text: $text, when: $when})")
            .param("id", exp.id.to_string())
            .param("text", exp.explanation)
            .param("when", exp.sensation.when.to_rfc3339());
        self.graph.run(q).await?;

        Ok(())
    }
}
