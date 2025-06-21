use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A raw observation in time.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Stimulus<T = ()> {
    /// The observed item or prior impression.
    pub what: T,
    /// When the observation occurred.
    pub timestamp: DateTime<Utc>,
}

impl<T> Stimulus<T> {
    /// Create a new stimulus with the current timestamp.
    pub fn new(what: T) -> Self {
        Self {
            what,
            timestamp: Utc::now(),
        }
    }
}

/// An interpretation of one or more stimuli.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Impression<T = ()> {
    /// Stimuli referenced by this impression.
    pub stimuli: Vec<Stimulus<T>>,
    /// Natural language summary of the impression.
    pub summary: String,
    /// Optional emotional tag.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emoji: Option<String>,
    /// When the impression was made.
    pub timestamp: DateTime<Utc>,
}

impl<T> Impression<T> {
    /// Create a new impression from `stimuli` with a textual `summary`.
    ///
    /// # Examples
    /// ```
    /// use psyche::model::{Stimulus, Impression};
    /// let stim = Stimulus::new("hi");
    /// let imp = Impression::new(vec![stim], "greeting", None::<String>);
    /// assert_eq!(imp.summary, "greeting");
    /// ```
    pub fn new(
        stimuli: Vec<Stimulus<T>>,
        summary: impl Into<String>,
        emoji: Option<impl Into<String>>,
    ) -> Self {
        Self {
            stimuli,
            summary: summary.into(),
            emoji: emoji.map(|e| e.into()),
            timestamp: Utc::now(),
        }
    }
}

/// A remembered impression stored with vector metadata.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Experience<T = ()> {
    /// Underlying impression being stored.
    #[serde(flatten)]
    pub impression: Impression<T>,
    /// Embedding vector derived from the summary.
    pub embedding: Vec<f32>,
    /// Unique identifier for recall.
    pub id: Uuid,
}

impl<T> Experience<T> {
    /// Create a new experience from an `impression` and its `embedding`.
    pub fn new(impression: Impression<T>, embedding: Vec<f32>) -> Self {
        Self {
            impression,
            embedding,
            id: Uuid::new_v4(),
        }
    }
}
