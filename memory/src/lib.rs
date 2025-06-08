//! In-memory abstractions for graph and vector databases used to store
//! embeddings of faces and sentences.
//! Components implementing [`MemoryComponent`] process sensations and
//! record them in a [`Memory`] instance.
use async_trait::async_trait;
use psyche::{Experience, Sensation};

/// Trait abstracting graph databases.
pub trait Graph {
    /// Link a face vector entry to a person node.
    fn link_face(&mut self, person: &str, face_id: usize);
}

/// Trait abstracting vector stores.
pub trait VectorDb {
    /// Insert a vector and return an identifier for later lookup.
    fn insert(&mut self, vector: Vec<f32>) -> usize;
}

/// Composite memory with connections to a graph and two vector databases.
#[derive(Default)]
pub struct Memory<G: Graph, F: VectorDb, S: VectorDb> {
    /// Graph database connection.
    pub graph: G,
    /// Vector database for facial embeddings.
    pub faces: F,
    /// Vector database for sentence embeddings.
    pub sentences: S,
}

impl<G: Graph, F: VectorDb, S: VectorDb> Memory<G, F, S> {
    /// Create a new memory from the provided backends.
    pub fn new(graph: G, faces: F, sentences: S) -> Self {
        Self {
            graph,
            faces,
            sentences,
        }
    }
}

/// Component capable of remembering sensations and experiences.
#[async_trait]
pub trait MemoryComponent<G: Graph, F: VectorDb, S: VectorDb> {
    /// Type of sensation data this component handles.
    type Input;

    /// Store the given sensation and experience inside the memory.
    async fn remember(
        &mut self,
        memory: &mut Memory<G, F, S>,
        sensation: Sensation<Self::Input>,
        experience: Experience,
    ) -> anyhow::Result<()>;
}

/// Component remembering human faces.
#[derive(Default)]
pub struct FaceMemory;

impl FaceMemory {
    /// Simulate encoding a JPEG image into a facial embedding.
    async fn encode_face(_data: &[u8]) -> Vec<f32> {
        vec![0.0; 128]
    }
}

#[async_trait]
impl<G, F, S> MemoryComponent<G, F, S> for FaceMemory
where
    G: Graph + Send,
    F: VectorDb + Send,
    S: VectorDb + Send,
{
    type Input = Vec<u8>;

    async fn remember(
        &mut self,
        memory: &mut Memory<G, F, S>,
        sensation: Sensation<Self::Input>,
        experience: Experience,
    ) -> anyhow::Result<()> {
        let vector = Self::encode_face(&sensation.what).await;
        let face_id = memory.faces.insert(vector);
        memory.graph.link_face(&experience.sentence, face_id);
        Ok(())
    }
}

/// Simple in-memory graph used for testing.
#[derive(Default)]
pub struct MockGraph {
    pub links: Vec<(String, usize)>,
}

impl Graph for MockGraph {
    fn link_face(&mut self, person: &str, face_id: usize) {
        self.links.push((person.to_string(), face_id));
    }
}

/// Simple in-memory vector store used for testing.
#[derive(Default)]
pub struct MockVectorDb {
    pub vectors: Vec<Vec<f32>>,
}

impl VectorDb for MockVectorDb {
    fn insert(&mut self, vector: Vec<f32>) -> usize {
        self.vectors.push(vector);
        self.vectors.len() - 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn face_memory_records_link() {
        let mut memory = Memory::new(
            MockGraph::default(),
            MockVectorDb::default(),
            MockVectorDb::default(),
        );
        let sensation = Sensation::new(vec![1, 2, 3]);
        let exp = Experience::new("Jake McJakerson");
        let mut component = FaceMemory::default();

        component
            .remember(&mut memory, sensation, exp.clone())
            .await
            .unwrap();

        assert_eq!(memory.faces.vectors.len(), 1);
        assert_eq!(memory.graph.links, vec![(exp.sentence, 0)]);
    }
}
