pub use psyche::{
    BrowserMotion, ConversationEntry, GeoLoc, Thought, WillTypeScriptExecution,
    WillTypeScriptResult, WitReport,
};
use serde::{Deserialize, Serialize};
#[cfg(feature = "ts")]
use ts_rs::{Dependency, TS};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "PascalCase", content = "data")]
/// Message envelope exchanged between the web client and server.
///
/// Each variant represents a distinct event that can occur during
/// conversation. Serialization uses an external `type` tag.
///
/// ```
/// use shared::WsPayload;
/// let msg = WsPayload::Text { text: "hi".into(), at: None };
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
        #[serde(default, skip_serializing_if = "Option::is_none")]
        at: Option<String>,
    },
    Echo {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        at: Option<String>,
    },
    SpeechPlayback {
        text: String,
        status: SpeechPlaybackStatus,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        at: Option<String>,
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
    Motion {
        data: BrowserMotion,
        at: Option<String>,
    },
    Sense {
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
    /// Raw streaming text from the language model.
    Chunk(String),
    /// Initial system prompt for the conversation.
    SystemPrompt(String),
    /// A single entry in the conversation log.
    ConversationEntry(ConversationEntry),
    /// Batch update of the entire conversation context.
    FullHistory(Thought),
}

#[cfg(feature = "ts")]
impl TS for WsPayload {
    const EXPORT_TO: Option<&'static str> = Some("../frontend/dist/ws_message.ts");

    fn name() -> String {
        "WsPayload".into()
    }

    fn decl() -> String {
        format!(
            r#"interface GeoLoc {{
  longitude: number;
  latitude: number;
  observed_at?: string;
}}

interface MotionVector {{
  x?: number;
  y?: number;
  z?: number;
}}

interface DeviceOrientation {{
  alpha?: number;
  beta?: number;
  gamma?: number;
  absolute?: boolean;
}}

interface BrowserMotion {{
  acceleration?: MotionVector;
  acceleration_including_gravity?: MotionVector;
  rotation_rate?: DeviceOrientation;
  orientation?: DeviceOrientation;
  interval?: number;
  observed_at?: string;
}}

interface AudioData {{
  base64: string;
  mime: string;
  sample_rate?: number;
  channels?: number;
}}

type SpeechPlaybackStatus = "Started" | "Finished" | "Interrupted";

interface WitReport {{
  name: string;
  prompt: string;
  output: string;
}}

interface WillTypeScriptResult {{
  command: string;
  output: string;
}}

interface WillTypeScriptExecution {{
  source: string;
  timestamp: string;
  results: WillTypeScriptResult[];
}}

interface ConversationEntry {{
  role: string;
  content: string;
  timestamp: string;
}}

interface Thought {{
  system_prompt: string;
  history: ConversationEntry[];
  report?: WitReport | null;
  typescript?: WillTypeScriptExecution | null;
  source_sensation_ids?: string[];
}}

type {} = {};"#,
            Self::name(),
            Self::inline()
        )
    }

    fn inline() -> String {
        r#"{ type: "Say"; data: { words: string; audio?: string | null } }
  | { type: "Emote"; data: string }
  | { type: "Think"; data: WitReport }
  | { type: "Text"; data: { text: string; at?: string } }
  | { type: "Echo"; data: { text: string; at?: string } }
  | { type: "SpeechPlayback"; data: { text: string; status: SpeechPlaybackStatus; at?: string } }
  | { type: "See"; data: { data: string; at?: string | null } }
  | { type: "Hear"; data: { data: AudioData; at?: string | null } }
  | { type: "Geolocate"; data: { data: GeoLoc; at?: string } }
  | { type: "Motion"; data: { data: BrowserMotion; at?: string } }
  | { type: "Sense"; data: { data: Record<string, any> } }
  | { type: "MotorCommand"; data: { target: string; command: string; args: Record<string, any> } }
  | { type: "Chunk"; data: string }
  | { type: "SystemPrompt"; data: string }
  | { type: "ConversationEntry"; data: ConversationEntry }
  | { type: "FullHistory"; data: Thought }"#
            .into()
    }

    fn dependencies() -> Vec<Dependency>
    where
        Self: 'static,
    {
        Vec::new()
    }

    fn transparent() -> bool {
        false
    }
}

#[cfg(all(test, feature = "ts"))]
#[test]
fn export_bindings_wspayload() {
    WsPayload::export().expect("export WsPayload TypeScript bindings");
}

#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AudioData {
    pub base64: String,
    pub mime: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sample_rate: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channels: Option<u32>,
}

#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum SpeechPlaybackStatus {
    Started,
    Finished,
    Interrupted,
}
