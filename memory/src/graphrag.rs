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

impl GraphRag {
    /// Store a face vector in a separate collection and create a graph node.
    pub async fn store_face(&self, id: uuid::Uuid, embedding: Vec<f32>) -> Result<(), MemoryError> {
        let point = PointStruct::new(id.as_u128() as u64, embedding, Payload::new());
        self.vector
            .upsert_points(UpsertPointsBuilder::new("faces", vec![point]).wait(true))
            .await?;
        let q = query("MERGE (f:Face {id: $id})")
            .param("id", id.to_string());
        self.graph.run(q).await?;
        Ok(())
    }

    /// Link a person's name to a face node.
    pub async fn link_person(&self, face_id: uuid::Uuid, name: &str) -> Result<(), MemoryError> {
        let q = query(
            "MERGE (p:Person {name: $name}) \nMATCH (f:Face {id: $id}) \nMERGE (p)-[:KNOWN_AS]->(f)",
        )
        .param("name", name)
        .param("id", face_id.to_string());
        self.graph.run(q).await?;
        Ok(())
    }

    /// Find the closest face vector and return its ID if within threshold.
    pub async fn find_face(&self, embedding: Vec<f32>, threshold: f32) -> Result<Option<uuid::Uuid>, MemoryError> {
        use qdrant_client::qdrant::SearchPointsBuilder;
        let search = SearchPointsBuilder::new("faces", embedding, 1).with_payload(false);
        let res = self.vector.search_points(search.build()).await?;
        if let Some(hit) = res.result.first() {
            if hit.score < threshold as f32 {
                if let Some(point_id) = &hit.id {
                    if let Some(opt) = &point_id.point_id_options {
                        if let qdrant_client::qdrant::point_id::PointIdOptions::Uuid(ref s) = opt {
                            return Ok(uuid::Uuid::parse_str(s).ok());
                        }
                    }
                }
            }
        }
        Ok(None)
    }
}
