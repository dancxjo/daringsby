use sensor::Sensation;
use uuid::Uuid;

/// A single stored memory event.
#[derive(Clone, Debug)]
pub struct Experience {
    pub id: Uuid,
    pub sensation: Sensation,
    pub explanation: String,
    pub embedding: Vec<f32>,
}

impl Experience {
    /// Create a new experience from a sensation and its metadata.
    pub fn new(sensation: Sensation, explanation: impl Into<String>, embedding: Vec<f32>) -> Self {
        Self {
            id: Uuid::new_v4(),
            sensation,
            explanation: explanation.into(),
            embedding,
        }
    }
}
