use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Newline-delimited JSON event emitted by the standalone face capture service.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MediaEvent {
    Image {
        sequence: u64,
        mime: String,
        base64: String,
        content_id: String,
        captured_at: Option<String>,
        received_at: DateTime<Utc>,
    },
    AudioLine {
        sequence: u64,
        mime: String,
        base64: String,
        sample_rate: u32,
        channels: u16,
        started_at: DateTime<Utc>,
        ended_at: DateTime<Utc>,
        duration_ms: u64,
        received_at: DateTime<Utc>,
    },
}
