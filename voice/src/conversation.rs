//! Lightweight conversation history tracker.
//!
//! The [`Conversation`] struct stores a bounded list of [`Message`]s exchanged
//! between Pete (the assistant) and the user. This context is used by
//! [`ChatVoice`](crate::ChatVoice) when crafting prompts.

use serde::{Deserialize, Serialize};

/// Speaker role of a [`Message`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    Assistant,
    User,
}

/// Single utterance in a conversation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

/// Rolling window of recent [`Message`]s.
pub struct Conversation {
    messages: Vec<Message>,
    max_len: usize,
}

impl Conversation {
    /// Create an empty conversation with a maximum length.
    pub fn new(max_len: usize) -> Self {
        Self {
            messages: Vec::new(),
            max_len,
        }
    }

    /// Append a new message and drop oldest if capacity is exceeded.
    pub fn push(&mut self, role: Role, content: impl Into<String>) {
        self.messages.push(Message {
            role,
            content: content.into(),
        });
        if self.messages.len() > self.max_len {
            let excess = self.messages.len() - self.max_len;
            self.messages.drain(0..excess);
        }
    }

    /// Return the slice of currently stored messages.
    pub fn tail(&self) -> &[Message] {
        &self.messages
    }
}
