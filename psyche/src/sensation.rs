use chrono::{DateTime, Utc};
use serde::Serialize;
use std::sync::Arc;
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
#[derive(Debug, Clone, Serialize)]
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

/// A coherent bundle of recently perceived sensations.
#[derive(Debug, Clone)]
pub struct Instant {
    /// Time the sensations were observed.
    pub at: DateTime<Utc>,
    /// The grouped sensations.
    pub sensations: Vec<Arc<Sensation>>,
}
