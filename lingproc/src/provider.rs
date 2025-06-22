//! Providers implementing the [`Doer`], [`Chatter`], and [`Vectorizer`] traits.

use crate::types::{ChatStream, Chatter, Doer, Instruction, Message, Role, Vectorizer};
use anyhow::{Result, anyhow};
use async_trait::async_trait;
use ollama_rs::{
    Ollama,
    generation::chat::{ChatMessage, request::ChatMessageRequest},
    generation::embeddings::request::{EmbeddingsInput, GenerateEmbeddingsRequest},
};
use std::time::Duration;
use tokio_stream::StreamExt;
use tracing::{debug, info, warn};

/// Provider backed by an Ollama server.
#[derive(Clone)]
pub struct OllamaProvider {
    client: Ollama,
    model: String,
}

impl OllamaProvider {
    /// Create a new provider for `model` hosted at `host`.
    pub fn new(host: impl AsRef<str>, model: impl Into<String>) -> Result<Self> {
        let host_ref = host.as_ref();
        let model = model.into();
        let client = Ollama::try_new(host_ref)?;
        info!(%host_ref, %model, "creating Ollama provider");
        Ok(Self { client, model })
    }
}

#[async_trait]
impl Doer for OllamaProvider {
    /// Follow an instruction via the Ollama API.
    async fn follow(&self, instruction: Instruction) -> Result<String> {
        use ollama_rs::generation::images::Image;
        let Instruction { command, images } = instruction;
        info!(%command, image_count = images.len(), "ollama follow");
        debug!(%command, image_count = images.len(), "ollama follow request");

        let mut msg = ChatMessage::user(command);
        if !images.is_empty() {
            let imgs: Vec<Image> = images
                .into_iter()
                .map(|i| Image::from_base64(i.base64))
                .collect();
            msg = msg.with_images(imgs);
        }
        let req = ChatMessageRequest::new(self.model.clone(), vec![msg]);
        let res = self.client.send_chat_messages(req).await?;
        debug!(response = %res.message.content, "ollama follow response");
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
        info!(history_len = history.len(), "ollama chat");
        debug!(%system_prompt, ?history, "ollama chat request");
        let req = ChatMessageRequest::new(self.model.clone(), msgs);
        let stream = self
            .client
            .send_chat_messages_stream(req)
            .await?
            .map(|res| match res {
                Ok(r) => {
                    let chunk = r.message.content;
                    debug!(%chunk, "ollama chat chunk");
                    Ok(chunk)
                }
                Err(e) => {
                    debug!(error = ?e, "ollama stream error");
                    Err(anyhow!("ollama stream error"))
                }
            });
        Ok(Box::pin(stream))
    }

    async fn update_prompt_context(&self, _context: &str) {}
}

#[async_trait]
impl Vectorizer for OllamaProvider {
    /// Request text embeddings from Ollama.
    async fn vectorize(&self, text: &str) -> Result<Vec<f32>> {
        info!(len = text.len(), "ollama vectorize");
        debug!(?text, "ollama vectorize request");
        let mut attempts = 0;
        loop {
            attempts += 1;
            let req =
                GenerateEmbeddingsRequest::new(self.model.clone(), EmbeddingsInput::from(text));
            match self.client.generate_embeddings(req).await {
                Ok(res) => {
                    debug!(
                        embedding_len = res.embeddings.len(),
                        "ollama vectorize response"
                    );
                    return Ok(res.embeddings.into_iter().next().unwrap_or_default());
                }
                Err(e) => {
                    if let ollama_rs::error::OllamaError::ReqwestError(ref re) = e {
                        if re.is_connect() {
                            warn!("🤖 vectorize failed: {}", re);
                            if attempts < 2 {
                                tokio::time::sleep(Duration::from_secs(1)).await;
                                continue;
                            }
                        }
                    }
                    return Err(anyhow!(e));
                }
            }
        }
    }
}
