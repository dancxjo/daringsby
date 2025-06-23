use psyche::{GeoLoc, WitReport};
use serde::{Deserialize, Serialize};
#[cfg(feature = "ts")]
use ts_rs::TS;

#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../frontend/dist/ws_message.ts")
)]
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "PascalCase", content = "data")]
/// Message envelope exchanged between the web client and server.
///
/// Each variant represents a distinct event that can occur during
/// conversation. Serialization uses an external `type` tag.
///
/// ```
/// use shared::MessageType;
/// let msg = MessageType::Text("hi".into());
/// let json = serde_json::to_string(&msg).unwrap();
/// assert!(json.contains("\"Text\""));
/// ```
pub enum WsPayload {
    Say {
        /// Spoken words.
        words: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        audio: Option<String>,
    },
    /// Change in emotional expression as an emoji.
    Emote(String),
    Think(WitReport),
    Text {
        text: String,
    },
    Echo {
        text: String,
    },
    See {
        data: String,
        at: Option<String>,
    },
    Hear {
        data: AudioData,
        at: Option<String>,
    },
    Geolocate {
        data: GeoLoc,
        at: Option<String>,
    },
    Sense {
        #[cfg_attr(feature = "ts", ts(type = "Record<string, any>"))]
        data: serde_json::Value,
    },
    MotorCommand {
        /// Target device or subsystem.
        target: String,
        /// Command verb.
        command: String,
        /// Additional arguments.
        args: serde_json::Value,
    },
}

#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AudioData {
    pub base64: String,
    pub mime: String,
}
