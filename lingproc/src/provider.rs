//! Providers implementing the [`Doer`], [`Chatter`], and [`Vectorizer`] traits.

use crate::types::{ChatStream, Chatter, Doer, Instruction, Message, Role, Vectorizer};
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
    /// Follow an instruction via the Ollama API.
    async fn follow(&self, instruction: Instruction) -> Result<String> {
        use ollama_rs::generation::images::Image;

        let mut msg = ChatMessage::user(instruction.command);
        if !instruction.images.is_empty() {
            let images: Vec<Image> = instruction
                .images
                .into_iter()
                .map(|i| Image::from_base64(i.base64))
                .collect();
            msg = msg.with_images(images);
        }
        let req = ChatMessageRequest::new(self.model.clone(), vec![msg]);
        let res = self.client.send_chat_messages(req).await?;
        Ok(res.message.content)
    }
}

#[async_trait]
impl Chatter for OllamaProvider {
    async fn chat(&self, system_prompt: &str, history: &[Message]) -> Result<ChatStream> {
        let mut msgs = Vec::with_capacity(history.len() + 1);
        msgs.push(ChatMessage::system(system_prompt.to_string()));
        for m in history {
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

    async fn update_prompt_context(&self, _context: &str) {}
}

#[async_trait]
impl Vectorizer for OllamaProvider {
    /// Request text embeddings from Ollama.
    async fn vectorize(&self, text: &str) -> Result<Vec<f32>> {
        let req = GenerateEmbeddingsRequest::new(self.model.clone(), EmbeddingsInput::from(text));
        let res = self.client.generate_embeddings(req).await?;
        Ok(res.embeddings.into_iter().next().unwrap_or_default())
    }
}
