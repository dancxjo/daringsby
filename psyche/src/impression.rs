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
