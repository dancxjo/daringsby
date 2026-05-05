use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
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
    HeardOwnVoice {
        text: String,
        occurred_at: DateTime<Utc>,
    },
    /// The user spoke to the assistant.
    HeardUserVoice {
        text: String,
        occurred_at: DateTime<Utc>,
    },
    /// Arbitrary input that the assistant can process
    Of {
        payload: Box<dyn std::any::Any + Send + Sync>,
        occurred_at: DateTime<Utc>,
    },
}

impl Sensation {
    /// Record Pete hearing his own speech at the moment it occurred.
    pub fn heard_own_voice(text: impl Into<String>) -> Self {
        Self::heard_own_voice_at(text, Utc::now())
    }

    /// Record Pete hearing his own speech with an externally supplied occurrence time.
    pub fn heard_own_voice_at(text: impl Into<String>, occurred_at: DateTime<Utc>) -> Self {
        Self::HeardOwnVoice {
            text: text.into(),
            occurred_at,
        }
    }

    /// Record Pete hearing user speech at the moment it occurred.
    pub fn heard_user_voice(text: impl Into<String>) -> Self {
        Self::heard_user_voice_at(text, Utc::now())
    }

    /// Record Pete hearing user speech with an externally supplied occurrence time.
    pub fn heard_user_voice_at(text: impl Into<String>, occurred_at: DateTime<Utc>) -> Self {
        Self::HeardUserVoice {
            text: text.into(),
            occurred_at,
        }
    }

    /// Record arbitrary sensory data at the moment it occurred.
    pub fn of<T>(payload: T) -> Self
    where
        T: std::any::Any + Send + Sync + 'static,
    {
        Self::of_at(payload, Utc::now())
    }

    /// Record arbitrary sensory data with an externally supplied occurrence time.
    pub fn of_at<T>(payload: T, occurred_at: DateTime<Utc>) -> Self
    where
        T: std::any::Any + Send + Sync + 'static,
    {
        Self::Of {
            payload: Box::new(payload),
            occurred_at,
        }
    }

    /// The time this sensation first happened in the real world.
    pub fn occurred_at(&self) -> DateTime<Utc> {
        match self {
            Self::HeardOwnVoice { occurred_at, .. }
            | Self::HeardUserVoice { occurred_at, .. }
            | Self::Of { occurred_at, .. } => *occurred_at,
        }
    }
}

impl Clone for Sensation {
    fn clone(&self) -> Self {
        match self {
            Self::HeardOwnVoice { text, occurred_at } => Self::HeardOwnVoice {
                text: text.clone(),
                occurred_at: *occurred_at,
            },
            Self::HeardUserVoice { text, occurred_at } => Self::HeardUserVoice {
                text: text.clone(),
                occurred_at: *occurred_at,
            },
            Self::Of { occurred_at, .. } => Self::Of {
                payload: Box::new(()),
                occurred_at: *occurred_at,
            },
        }
    }
}
