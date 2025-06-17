use serde::{Deserialize, Serialize};

/// A structured cognitive unit summarizing Pete's perception at a moment in
/// time.
///
/// This is a memory object suitable for embedding and storage.
///
/// - `headline`: one-sentence summary, suitable for Qdrant embedding.
/// - `details`: optional paragraph summary, used for narrative reflection.
/// - `raw_data`: arbitrary serializable data, stored in Neo4j.
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

impl<T> Impression<T> {
    /// Construct a new [`Impression`].
    ///
    /// # Examples
    ///
    /// ```
    /// use psyche::Impression;
    /// let imp = Impression::new("a" , Some("b"), 42);
    /// assert_eq!(imp.headline, "a");
    /// ```
    pub fn new(
        headline: impl Into<String>,
        details: Option<impl Into<String>>,
        raw_data: T,
    ) -> Self {
        Self {
            headline: headline.into(),
            details: details.map(|d| d.into()),
            raw_data,
        }
    }
}
