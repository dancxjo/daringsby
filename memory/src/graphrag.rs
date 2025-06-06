use async_trait::async_trait;
use qdrant_client::{qdrant::{PointStruct, UpsertPointsBuilder}, Qdrant, Payload};
use neo4rs::{Graph, query};

use crate::{Experience, MemoryError};

pub struct GraphRag {
    vector: Qdrant,
    graph: Graph,
    collection: String,
}

impl GraphRag {
    pub fn new(vector_url: &str, graph_uri: &str, user: &str, pass: &str) -> Result<Self, MemoryError> {
        let vector = Qdrant::from_url(vector_url).build()?;
        let graph = Graph::new(graph_uri, user, pass)?;
        Ok(Self { vector, graph, collection: "experiences".into() })
    }
}

#[async_trait]
pub trait Memory {
    async fn store(&self, exp: Experience) -> Result<(), MemoryError>;
}

#[async_trait]
impl Memory for GraphRag {
    async fn store(&self, exp: Experience) -> Result<(), MemoryError> {
        let point = PointStruct::new(exp.id.as_u128() as u64, exp.embedding.clone(), Payload::new());
        self.vector
            .upsert_points(
                UpsertPointsBuilder::new(&self.collection, vec![point]).wait(true),
            )
            .await?;

        let q = query("MERGE (e:Experience {id: $id, text: $text, when: $when})")
            .param("id", exp.id.to_string())
            .param("text", exp.explanation)
            .param("when", exp.sensation.when.to_rfc3339());
        self.graph.run(q).await?;

        Ok(())
    }
}
