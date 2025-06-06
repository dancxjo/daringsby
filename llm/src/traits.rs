use async_trait::async_trait;
use futures_core::Stream;
use std::pin::Pin;
use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LLMCapability {
    Chat,
    Embed,
    Vision,
    Code,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LLMAttribute {
    Fast,
    Slow,
    LowMemory,
    Local,
    Remote,
}

#[derive(Debug, Error)]
pub enum LLMError {
    #[error("network error: {0}")]
    Network(String),
    #[error("invalid response")]
    InvalidResponse,
    #[error("model not found")]
    ModelNotFound,
}

#[async_trait]
pub trait LLMClient: Send + Sync {
    async fn stream_chat(
        &self,
        model: &str,
        prompt: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String, LLMError>> + Send>>, LLMError>;

    async fn embed(&self, model: &str, input: &str) -> Result<Vec<f32>, LLMError>;
}
