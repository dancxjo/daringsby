use anyhow::Result;
use async_trait::async_trait;
use std::pin::Pin;
use tokio_stream::Stream;

/// Processes instructions and returns textual responses.
#[async_trait]
pub trait Doer: Send + Sync {
    async fn follow(&self, instruction: &str) -> Result<String>;
}

/// Speaker roles for a chat message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Role {
    Assistant,
    User,
}

/// Message in a chat exchange.
#[derive(Clone, Debug)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

impl Message {
    /// Create a new user message.
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
        }
    }

    /// Create a new assistant message.
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
        }
    }
}

/// Stream of chat response chunks.
pub type ChatStream = Pin<Box<dyn Stream<Item = Result<String>> + Send>>;

#[async_trait]
pub trait Chatter: Send + Sync {
    async fn chat(&self, system_prompt: &str, history: &[Message]) -> Result<ChatStream>;
}

/// Produces vector representations of text.
#[async_trait]
pub trait Vectorizer: Send + Sync {
    async fn vectorize(&self, text: &str) -> Result<Vec<f32>>;
}
