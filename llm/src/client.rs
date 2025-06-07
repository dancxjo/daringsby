//! HTTP client for interacting with an Ollama language model server.
//!
//! This module provides the [`OllamaClient`] type which implements the
//! [`LLMClient`] trait. It streams chat responses and
//! requests embeddings from a running Ollama instance.

use crate::traits::{LLMClient, LLMError};
use async_trait::async_trait;
use futures_core::Stream;
use std::pin::Pin;
use tokio_stream::StreamExt;

use ollama_rs::{
    generation::{
        completion::request::GenerationRequest, embeddings::request::GenerateEmbeddingsRequest,
    },
    Ollama,
};

pub struct OllamaClient {
    inner: Ollama,
}

impl OllamaClient {
    pub fn new(base_url: impl AsRef<str>) -> Self {
        Self {
            inner: Ollama::try_new(base_url.as_ref()).unwrap(),
        }
    }
}

#[async_trait]
impl LLMClient for OllamaClient {
    async fn stream_chat(
        &self,
        model: &str,
        prompt: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String, LLMError>> + Send>>, LLMError> {
        let req = GenerationRequest::new(model.to_string(), prompt.to_string());
        let stream = self
            .inner
            .generate_stream(req)
            .await
            .map_err(|e| LLMError::Network(e.to_string()))?;
        let mapped = stream.map(|res| {
            res.map_err(|e| LLMError::Network(e.to_string()))
                .map(|chunk| {
                    chunk
                        .into_iter()
                        .map(|c| c.response)
                        .collect::<Vec<_>>()
                        .join("")
                })
        });
        Ok(Box::pin(mapped))
    }

    async fn embed(&self, model: &str, input: &str) -> Result<Vec<f32>, LLMError> {
        let req = GenerateEmbeddingsRequest::new(model.to_string(), input.into());
        let res = self
            .inner
            .generate_embeddings(req)
            .await
            .map_err(|e| LLMError::Network(e.to_string()))?;
        Ok(res.embeddings.into_iter().next().unwrap_or_default())
    }
}
