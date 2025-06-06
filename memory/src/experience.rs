use sensor::Sensation;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct Experience {
    pub id: Uuid,
    pub sensation: Sensation,
    pub explanation: String,
    pub embedding: Vec<f32>,
}

impl Experience {
    pub fn new(sensation: Sensation, explanation: impl Into<String>, embedding: Vec<f32>) -> Self {
        Self {
            id: Uuid::new_v4(),
            sensation,
            explanation: explanation.into(),
            embedding,
        }
    }
}
