use serde::{Deserialize, Serialize};

#[cfg_attr(all(target_arch = "wasm32", feature = "ts"), derive(ts_rs::TS))]
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", content = "data")]
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
pub enum MessageType {
    /// Assistant speech with optional audio.
    Say {
        /// Spoken words.
        words: String,
        /// Base64 WAV data of the speech.
        audio: String,
    },
    /// Change in emotional expression as an emoji.
    Emote(String),
    /// Debug thought from a Wit.
    Think(String),
    /// Text the assistant heard itself say.
    Heard(String),
    /// Arbitrary user text.
    Text(String),
    /// Base64-encoded image data URL.
    See(String),
    /// Raw audio fragment.
    Hear { base64: String, mime: String },
    /// Geographic coordinates.
    Geolocate { longitude: f64, latitude: f64 },
    /// Command for a motor implementation.
    MotorCommand {
        /// Target device or subsystem.
        target: String,
        /// Command verb.
        command: String,
        /// Additional arguments.
        args: serde_json::Value,
    },
}
