//! Language-related helpers and abstractions.
//!
//! This module defines traits for interacting with language models and provides
//! an `OllamaProvider` implementation using the [`ollama-rs`] crate.
//!
//! ```no_run
//! use psyche::ling::{OllamaProvider, Narrator, Voice, Vectorizer};
//! use psyche::Psyche;
//!
//! # async fn try_it() -> anyhow::Result<()> {
//! let narrator = OllamaProvider::new("http://localhost:11434", "mistral")?;
//! let voice = OllamaProvider::new("http://localhost:11434", "mistral")?;
//! let vectorizer = OllamaProvider::new("http://localhost:11434", "mistral")?;
//! let psyche = Psyche::new(Box::new(narrator), Box::new(voice), Box::new(vectorizer));
//! psyche.run();
//! # Ok(()) }
//! ```
use async_trait::async_trait;
use anyhow::Result;
use ollama_rs::{Ollama, generation::chat::{ChatMessage, request::ChatMessageRequest}, generation::embeddings::{request::{GenerateEmbeddingsRequest, EmbeddingsInput}}};

/// Processes instructions and returns textual responses.
#[async_trait]
pub trait Narrator: Send + Sync {
    async fn follow(&self, instruction: &str) -> Result<String>;
}

/// Exchanges conversational messages.
#[derive(Clone, Debug)]
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
        Self { role: Role::User, content: content.into() }
    }

    /// Create a new assistant message.
    pub fn assistant(content: impl Into<String>) -> Self {
        Self { role: Role::Assistant, content: content.into() }
    }
}

#[async_trait]
pub trait Voice: Send + Sync {
    async fn chat(&self, system_prompt: &str, history: &[Message]) -> Result<String>;
}

/// Produces vector representations of text.
#[async_trait]
pub trait Vectorizer: Send + Sync {
    async fn vectorize(&self, text: &str) -> Result<Vec<f32>>;
}

/// Provider backed by an Ollama server.
#[derive(Clone)]
pub struct OllamaProvider {
    client: Ollama,
    model: String,
}

impl OllamaProvider {
    /// Create a new provider for `model` hosted at `host`.
    pub fn new(host: impl AsRef<str>, model: impl Into<String>) -> Result<Self> {
        let client = Ollama::try_new(host.as_ref())?;
        Ok(Self { client, model: model.into() })
    }
}

#[async_trait]
impl Narrator for OllamaProvider {
    async fn follow(&self, instruction: &str) -> Result<String> {
        let req = ChatMessageRequest::new(
            self.model.clone(),
            vec![ChatMessage::user(instruction.to_string())],
        );
        let res = self.client.send_chat_messages(req).await?;
        Ok(res.message.content)
    }
}

#[async_trait]
impl Voice for OllamaProvider {
    async fn chat(&self, system_prompt: &str, history: &[Message]) -> Result<String> {
        let mut msgs = Vec::with_capacity(history.len() + 1);
        msgs.push(ChatMessage::system(system_prompt.to_string()));
        for m in history {
            let converted = match m.role {
                Role::Assistant => ChatMessage::assistant(m.content.clone()),
                Role::User => ChatMessage::user(m.content.clone()),
            };
            msgs.push(converted);
        }
        let req = ChatMessageRequest::new(self.model.clone(), msgs);
        let res = self.client.send_chat_messages(req).await?;
        Ok(res.message.content)
    }
}

#[async_trait]
impl Vectorizer for OllamaProvider {
    async fn vectorize(&self, text: &str) -> Result<Vec<f32>> {
        let req = GenerateEmbeddingsRequest::new(self.model.clone(), EmbeddingsInput::from(text));
        let res = self.client.generate_embeddings(req).await?;
        Ok(res.embeddings.into_iter().next().unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Dummy;

    #[async_trait]
    impl Narrator for Dummy {
        async fn follow(&self, i: &str) -> Result<String> {
            Ok(format!("f:{i}"))
        }
    }

    #[async_trait]
    impl Voice for Dummy {
        async fn chat(&self, _s: &str, h: &[Message]) -> Result<String> {
            Ok(format!("c:{}", h.len()))
        }
    }

    #[async_trait]
    impl Vectorizer for Dummy {
        async fn vectorize(&self, t: &str) -> Result<Vec<f32>> {
            Ok(vec![t.len() as f32])
        }
    }

    #[tokio::test]
    async fn traits_work() {
        let d = Dummy;
        assert_eq!(d.follow("x").await.unwrap(), "f:x");
        let hist = vec![Message::user("hi"), Message::assistant("hey")];
        assert_eq!(d.chat("sys", &hist).await.unwrap(), "c:2");
        assert_eq!(d.vectorize("ab").await.unwrap(), vec![2.0]);
    }
}
