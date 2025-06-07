//! Abstractions over concrete LLM backends used by the voice agent.
//!
//! The types in this module define a small interface for streaming chat
//! completions and embeddings. A default [`OllamaClient`] is provided along with
//! a [`MockModelClient`] used in tests.

use async_trait::async_trait;
use futures_core::Stream;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use thiserror::Error;

pub mod registry;
pub mod scheduler;

/// Features that a model can provide.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Capability {
    Chat,
    Embedding,
    Vision,
    Code,
}

/// Qualitative attributes describing a model or server.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Attribute {
    Fast,
    Slow,
    LowMemory,
    HighMemory,
}

/// Errors originating from a [`ModelClient`].
#[derive(Debug, Error)]
pub enum ModelError {
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("invalid response")]
    InvalidResponse,
}

/// Interface for streaming chat completions and embeddings.
#[async_trait]
pub trait ModelClient {
    async fn stream_chat(
        &self,
        model: &str,
        prompt: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String, ModelError>> + Send>>, ModelError>;

    async fn embed(&self, model: &str, input: &str) -> Result<Vec<f32>, ModelError>;
}

/// Implementation of [`ModelClient`] that talks to an Ollama server over HTTP.
pub struct OllamaClient {
    pub base_url: String,
    client: reqwest::Client,
}

impl OllamaClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl ModelClient for OllamaClient {
    async fn stream_chat(
        &self,
        model: &str,
        prompt: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String, ModelError>> + Send>>, ModelError> {
        let url = format!("{}/api/generate", self.base_url);
        let resp = self
            .client
            .post(url)
            .json(&serde_json::json!({"model": model, "prompt": prompt, "stream": true}))
            .send()
            .await?;
        let stream = resp.bytes_stream().map(|b| {
            b.map_err(ModelError::from).and_then(|chunk| {
                let text = String::from_utf8_lossy(&chunk);
                for line in text.lines() {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                        if let Some(resp) = v.get("response").and_then(|r| r.as_str()) {
                            return Ok(resp.to_string());
                        }
                    }
                }
                Err(ModelError::InvalidResponse)
            })
        });
        Ok(Box::pin(stream))
    }

    async fn embed(&self, model: &str, input: &str) -> Result<Vec<f32>, ModelError> {
        let url = format!("{}/api/embeddings", self.base_url);
        let resp = self
            .client
            .post(url)
            .json(&serde_json::json!({"model": model, "prompt": input}))
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;
        if let Some(arr) = resp.get("embedding").and_then(|e| e.as_array()) {
            let vec = arr
                .iter()
                .filter_map(|v| v.as_f64())
                .map(|f| f as f32)
                .collect();
            Ok(vec)
        } else {
            Err(ModelError::InvalidResponse)
        }
    }
}

/// Simple in-memory mock used in unit tests.
pub struct MockModelClient {
    pub responses: Vec<String>,
    pub embeddings: Vec<f32>,
}

impl MockModelClient {
    pub fn new(responses: Vec<String>, embeddings: Vec<f32>) -> Self {
        Self {
            responses,
            embeddings,
        }
    }
}

#[async_trait]
impl ModelClient for MockModelClient {
    async fn stream_chat(
        &self,
        _model: &str,
        _prompt: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String, ModelError>> + Send>>, ModelError> {
        let iter = self.responses.clone().into_iter().map(Ok);
        Ok(Box::pin(tokio_stream::iter(iter)))
    }

    async fn embed(&self, _model: &str, _input: &str) -> Result<Vec<f32>, ModelError> {
        Ok(self.embeddings.clone())
    }
}
