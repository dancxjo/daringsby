use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
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

    /// Stable graph identifier for this raw sensation.
    pub fn id(&self) -> String {
        match self {
            Self::HeardOwnVoice { text, occurred_at } => {
                let utterance_id = format!("utterance:self:{}:{text}", occurred_at.to_rfc3339());
                sensation_id("utterance", &utterance_id, occurred_at)
            }
            Self::HeardUserVoice { text, occurred_at } => {
                let utterance_id = format!("utterance:user:{}:{text}", occurred_at.to_rfc3339());
                sensation_id("utterance", &utterance_id, occurred_at)
            }
            Self::Of {
                payload,
                occurred_at,
            } => {
                if let Some(image) = payload.downcast_ref::<crate::ImageData>() {
                    sensation_id("image", &crate::image_content_id(image), occurred_at)
                } else if let Some(loc) = payload.downcast_ref::<crate::GeoLoc>() {
                    sensation_id("geolocation", &crate::geoloc_content_id(loc), occurred_at)
                } else if let Some(motion) = payload.downcast_ref::<crate::BrowserMotion>() {
                    sensation_id(
                        "browser_motion",
                        &crate::browser_motion_content_id(motion),
                        occurred_at,
                    )
                } else if let Some(geo_embedding) = payload.downcast_ref::<crate::GeoEmbedding>() {
                    let point_id = geo_embedding.vector_id.clone().unwrap_or_else(|| {
                        format!("geolocation-vector:{}", geo_embedding.geoloc_id)
                    });
                    let vector_id =
                        crate::wits::memory::qdrant_vector_node_id("geolocations", &point_id);
                    sensation_id("geolocation_embedding", &vector_id, occurred_at)
                } else if let Some(image_embedding) =
                    payload.downcast_ref::<crate::ImageEmbedding>()
                {
                    let point_id = image_embedding
                        .vector_id
                        .clone()
                        .unwrap_or_else(|| format!("image-vector:{}", image_embedding.image_id));
                    let vector_id = crate::wits::memory::qdrant_vector_node_id("images", &point_id);
                    sensation_id("image_embedding", &vector_id, occurred_at)
                } else {
                    #[cfg(feature = "face")]
                    if let Some(face) = payload.downcast_ref::<crate::FaceInfo>() {
                        return sensation_id("face", &face.face_id, occurred_at);
                    }
                    if let Some(voice) = payload.downcast_ref::<crate::VoiceInfo>() {
                        sensation_id("voice", &voice.clip_id, occurred_at)
                    } else if let Some(audio) = payload.downcast_ref::<crate::AudioClip>() {
                        sensation_id("audio", &crate::audio_clip_id(audio), occurred_at)
                    } else if let Some(heartbeat) = payload.downcast_ref::<crate::Heartbeat>() {
                        let id = format!("heartbeat:{}", heartbeat.timestamp.to_rfc3339());
                        sensation_id("heartbeat", &id, occurred_at)
                    } else if let Some(object) = payload.downcast_ref::<crate::ObjectInfo>() {
                        sensation_id("object", &object_info_id(object, occurred_at), occurred_at)
                    } else if let Some(value) = payload.downcast_ref::<Value>() {
                        sensation_id("json", &json_sensation_id(value, occurred_at), occurred_at)
                    } else if let Some(summary) =
                        payload.downcast_ref::<crate::CombobulationSummary>()
                    {
                        sensation_id(
                            "combobulation_summary",
                            &combobulation_summary_id(summary, occurred_at),
                            occurred_at,
                        )
                    } else {
                        let type_id = format!("{:?}", payload.as_ref().type_id());
                        let id =
                            format!("unknown-sensation:{type_id}:{}", occurred_at.to_rfc3339());
                        sensation_id("unknown", &id, occurred_at)
                    }
                }
            }
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

fn sensation_id(kind: &str, content_id: &str, occurred_at: &DateTime<Utc>) -> String {
    format!("sensation:{kind}:{content_id}:{}", occurred_at.to_rfc3339())
}

fn object_info_id(object: &crate::ObjectInfo, occurred_at: &DateTime<Utc>) -> String {
    format!(
        "object:{}:{}:{}",
        object.label.clone().unwrap_or_else(|| "unknown".into()),
        object.embedding.len(),
        occurred_at.to_rfc3339()
    )
}

fn json_sensation_id(value: &Value, occurred_at: &DateTime<Utc>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.to_string().as_bytes());
    hasher.update([0]);
    hasher.update(occurred_at.to_rfc3339().as_bytes());
    format!("json-sensation:sha256:{:x}", hasher.finalize())
}

fn combobulation_summary_id(
    summary: &crate::CombobulationSummary,
    occurred_at: &DateTime<Utc>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(summary.text.as_bytes());
    hasher.update([0]);
    hasher.update(occurred_at.to_rfc3339().as_bytes());
    format!("combobulation-summary:sha256:{:x}", hasher.finalize())
}
