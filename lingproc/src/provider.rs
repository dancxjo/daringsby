//! Providers implementing the [`Doer`], [`Chatter`], and [`Vectorizer`] traits.

use crate::types::{ChatContext, ChatStream, Chatter, Doer, Message, Role, Vectorizer};
use anyhow::{Result, anyhow};
use async_trait::async_trait;
use ollama_rs::{
    Ollama,
    generation::chat::{ChatMessage, request::ChatMessageRequest},
    generation::embeddings::request::{EmbeddingsInput, GenerateEmbeddingsRequest},
};
use tokio_stream::StreamExt;

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
        Ok(Self {
            client,
            model: model.into(),
        })
    }
}

#[async_trait]
impl Doer for OllamaProvider {
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
impl Chatter for OllamaProvider {
    async fn chat(&self, ctx: ChatContext<'_>) -> Result<ChatStream> {
        let mut msgs = Vec::with_capacity(ctx.history.len() + 1);
        msgs.push(ChatMessage::system(ctx.system_prompt.to_string()));
        for m in ctx.history {
            let m = match m.role {
                Role::Assistant => ChatMessage::assistant(m.content.clone()),
                Role::User => ChatMessage::user(m.content.clone()),
            };
            msgs.push(m);
        }
        let req = ChatMessageRequest::new(self.model.clone(), msgs);
        let stream = self
            .client
            .send_chat_messages_stream(req)
            .await?
            .map(|res| {
                res.map(|r| r.message.content)
                    .map_err(|_| anyhow!("ollama stream error"))
            });
        Ok(Box::pin(stream))
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
