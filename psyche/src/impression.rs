use serde::{Deserialize, Serialize};

/// A distilled perception captured by a Wit layer.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Impression<T> {
    /// One-sentence summary for vector storage.
    pub headline: String,
    /// Optional paragraph providing more detail.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    /// Serializable raw payload saved in the graph database.
    #[serde(rename = "rawData")]
    pub raw_data: T,
}
