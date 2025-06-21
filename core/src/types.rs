use serde::{Deserialize, Serialize};

/// Basic unit of input for the psyche.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Stimulus<T = serde_json::Value> {
    /// Arbitrary payload describing the stimulus.
    pub what: T,
    /// Millisecond timestamp when the stimulus occurred.
    pub timestamp: i64,
}

/// Wit's output summarising a set of stimuli.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Impression {
    /// Raw stimuli that led to the impression.
    pub stimuli: Vec<Stimulus>,
    /// Human friendly summary.
    pub summary: String,
    /// Optional emoji representing the impression.
    pub emoji: Option<String>,
    /// Millisecond timestamp of the impression.
    pub timestamp: i64,
}

/// Result of embedding an impression.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Experience {
    /// The impression itself.
    #[serde(flatten)]
    pub impression: Impression,
    /// Vector representation of the impression.
    pub embedding: Vec<f32>,
    /// Unique identifier used by memory stores.
    pub id: String,
}
