use chrono::{DateTime, Local, Utc};
use serde::{Deserialize, Serialize};
use std::fmt::Display;
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

    /// Return this stimulus timestamp in the host's local timezone.
    pub fn localized_timestamp(&self) -> String {
        localized_timestamp(self.timestamp)
    }
}

impl<T: Display> Stimulus<T> {
    /// Render this stimulus as a timestamped prompt list item.
    pub fn prompt_list_item(&self) -> String {
        format!("[{}] {}", self.localized_timestamp(), self.what)
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

    /// Return this impression timestamp in the host's local timezone.
    pub fn localized_timestamp(&self) -> String {
        localized_timestamp(self.timestamp)
    }
}

impl<T> Impression<T> {
    /// Render this impression summary as a timestamped prompt list item.
    pub fn prompt_list_item(&self) -> String {
        format!("[{}] {}", self.localized_timestamp(), self.summary)
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

    /// Return this experience timestamp in the host's local timezone.
    pub fn localized_timestamp(&self) -> String {
        self.impression.localized_timestamp()
    }

    /// Render this experience summary as a timestamped prompt list item.
    pub fn prompt_list_item(&self) -> String {
        format!(
            "[{}] {} ({})",
            self.localized_timestamp(),
            self.impression.summary,
            self.id
        )
    }
}

/// Format a UTC timestamp in the host's local timezone for LLM prompts.
pub fn localized_timestamp(timestamp: DateTime<Utc>) -> String {
    timestamp
        .with_timezone(&Local)
        .format("%Y-%m-%d %H:%M:%S %Z")
        .to_string()
}
