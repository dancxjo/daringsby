use serde::{Deserialize, Serialize};

#[cfg_attr(all(target_arch = "wasm32", feature = "ts"), derive(ts_rs::TS))]
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", content = "data")]
pub enum MessageType {
    Say {
        words: String,
        audio: String,
    },
    Emote(String),
    Think(String),
    Heard(String),
    Text(String),
    See(String),
    Hear {
        base64: String,
        mime: String,
    },
    Geolocate {
        longitude: f64,
        latitude: f64,
    },
    MotorCommand {
        target: String,
        command: String,
        args: serde_json::Value,
    },
}
