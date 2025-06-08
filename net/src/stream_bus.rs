use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

/// Role of a conversation message.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ConversationRole {
    Assistant,
    User,
}

/// Events published across Pete's streaming bus.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum StreamEvent {
    AsrPartial { transcript: String },
    AsrFinal { transcript: String },
    LlmThoughtFragment { content: String },
    LlmFinalResponse { content: String },
    /// Notification that a spoken response has begun.
    LlmBeginSay,
    /// Fragment of a spoken response as it streams.
    LlmSayFragment { content: String },
    /// Indicates the utterance is complete or was interrupted.
    LlmEndSay { complete: bool },
    TtsChunkReady { id: usize },
    PerceptionLog { text: String },
    MemoryUpdate { summary: String },
    ConsentCheck { ok: bool },
    /// Text Pete is about to say aloud.
    GoingToSay { text: String },
    /// Finalized line added to the conversation history.
    ConversationUpdate { role: ConversationRole, content: String },
}

/// Simple broadcast channel for streaming events.
pub struct StreamBus {
    tx: broadcast::Sender<StreamEvent>,
}

impl StreamBus {
    /// Create a new bus with the given capacity.
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx }
    }

    /// Subscribe to the stream.
    pub fn subscribe(&self) -> broadcast::Receiver<StreamEvent> {
        self.tx.subscribe()
    }

    /// Broadcast an event to all subscribers.
    pub fn send(&self, event: StreamEvent) -> Result<(), broadcast::error::SendError<StreamEvent>> {
        self.tx.send(event).map(|_| ())
    }
}
