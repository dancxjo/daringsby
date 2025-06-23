use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
#[cfg(feature = "ts")]
use ts_rs::TS;
/// Event types emitted by the [`Psyche`] during conversation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// A partial chunk of the assistant's response.
    StreamChunk(String),
    /// The assistant spoke a line of dialogue. Optional base64-encoded WAV audio accompanies the text.
    Speech { text: String, audio: Option<String> },
    /// The psyche's emotional expression changed.
    EmotionChanged(String),
}

/// Debug information emitted by a [`Wit`].
#[cfg_attr(feature = "ts", derive(TS))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WitReport {
    /// Name of the wit generating the prompt.
    pub name: String,
    /// Prompt sent to the language model.
    pub prompt: String,
    /// Final response returned by the model.
    pub output: String,
}

/// Inputs that can be sent to a running [`Psyche`].
#[derive(Debug)]
pub enum Sensation {
    /// The assistant's speech was heard.
    HeardOwnVoice(String),
    /// The user spoke to the assistant.
    HeardUserVoice(String),
    /// Arbitrary input that the assistant can process
    Of(Box<dyn std::any::Any + Send + Sync>),
}

impl Clone for Sensation {
    fn clone(&self) -> Self {
        match self {
            Self::HeardOwnVoice(t) => Self::HeardOwnVoice(t.clone()),
            Self::HeardUserVoice(t) => Self::HeardUserVoice(t.clone()),
            Self::Of(_) => Self::Of(Box::new(())),
        }
    }
}

/// A coherent bundle of recently perceived sensations.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Instant {
    /// Time the sensations were observed.
    pub at: DateTime<Utc>,
    /// The grouped sensations.
    #[serde(skip)]
    pub sensations: Vec<Arc<Sensation>>,
}
