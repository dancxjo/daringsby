//! Core traits and enums describing language model capabilities.
//!
//! Other crates use these abstractions to remain agnostic over the specific LLM
//! backend being used.

use async_trait::async_trait;
use futures_core::Stream;
use std::pin::Pin;
use thiserror::Error;

/// Features that a model can support.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LLMCapability {
    Chat,
    Embed,
    Vision,
    Code,
}

/// Qualitative attributes describing a server or model.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LLMAttribute {
    Fast,
    Slow,
    LowMemory,
    Local,
    Remote,
}

/// Errors produced by an [`LLMClient`].
#[derive(Debug, Error)]
pub enum LLMError {
    #[error("network error: {0}")]
    Network(String),
    #[error("invalid response")]
    InvalidResponse,
    #[error("model not found")]
    ModelNotFound,
    #[error("queue full")]
    QueueFull,
}

/// Interface for talking to a language model server.
#[async_trait]
pub trait LLMClient: Send + Sync {
    async fn stream_chat(
        &self,
        model: &str,
        prompt: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String, LLMError>> + Send>>, LLMError>;

    async fn embed(&self, model: &str, input: &str) -> Result<Vec<f32>, LLMError>;
}
