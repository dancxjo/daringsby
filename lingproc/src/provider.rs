use crate::types::{Chatter, Doer, LlmInstruction, Message, Role, TextStream, Vectorizer};
use anyhow::{Result, anyhow};
use async_trait::async_trait;
use ollama_rs::models::ModelOptions;
use ollama_rs::{
    Ollama,
    generation::chat::{ChatMessage, request::ChatMessageRequest},
    generation::embeddings::request::{EmbeddingsInput, GenerateEmbeddingsRequest},
};
use rand::Rng;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::time::timeout;
use tokio_stream::StreamExt;
use tracing::{debug, info, trace, warn};

/// Provider backed by one or more Ollama servers.
#[derive(Clone)]
pub struct OllamaProvider {
    clients: Vec<Ollama>,
    model: String,
    next: Arc<AtomicUsize>,
}

impl OllamaProvider {
    /// Create a new provider for `model` hosted at one or more `hosts`.
    pub fn new(
        hosts: impl IntoIterator<Item = impl AsRef<str>>,
        model: impl Into<String>,
    ) -> Result<Self> {
        let model = model.into();
        let mut clients = Vec::new();
        for host in hosts {
            let host_ref = host.as_ref();
            if host_ref.is_empty() {
                continue;
            }
            let client = Ollama::try_new(host_ref)?;
            info!(%host_ref, %model, "adding Ollama client to provider");
            clients.push(client);
        }
        if clients.is_empty() {
            return Err(anyhow!("at least one host must be provided"));
        }
        Ok(Self {
            clients,
            model,
            next: Arc::new(AtomicUsize::new(0)),
        })
    }

    /// Create a new provider using the given host(s) and model, falling back to
    /// sensible defaults when either is `None`.
    ///
    /// The host parameter can be a single URL or a comma-separated list of URLs.
    /// The default host is `http://localhost:11434` and the default model is
    /// `gpt-oss`.
    pub fn new_with_defaults(host: Option<&str>, model: Option<&str>) -> Result<Self> {
        let host_str = host.unwrap_or("http://localhost:11434");
        let model = model.unwrap_or("gpt-oss");
        let hosts: Vec<&str> = host_str.split(',').map(|s| s.trim()).collect();
        Self::new(hosts, model)
    }

    fn client(&self) -> &Ollama {
        let idx = self.next.fetch_add(1, Ordering::SeqCst) % self.clients.len();
        &self.clients[idx]
    }
}

#[async_trait]
impl Doer for OllamaProvider {
    /// Follow an instruction via the Ollama API.
    async fn follow(&self, instruction: LlmInstruction) -> Result<String> {
        use ollama_rs::generation::images::Image;
        let LlmInstruction { command, images } = instruction;
        debug!(
            model = %self.model,
            command_len = command.len(),
            image_count = images.len(),
            "ollama follow"
        );
        debug!(
            model = %self.model,
            prompt = %command,
            image_count = images.len(),
            "ollama follow request"
        );
        trace!(%command, image_count = images.len(), "ollama follow request");

        let mut msg = ChatMessage::user(command);
        if !images.is_empty() {
            let imgs: Vec<Image> = images
                .into_iter()
                .map(|i| Image::from_base64(i.base64))
                .collect();
            msg = msg.with_images(imgs);
        }
        let temperature = 0.8 + (rand::thread_rng().gen_range(-0.05..0.05));
        let options = ModelOptions::default().temperature(temperature);
        let req = ChatMessageRequest::new(self.model.clone(), vec![msg]).options(options);
        let res = self.client().send_chat_messages(req).await?;
        debug!(
            model = %self.model,
            response = %res.message.content,
            "ollama follow response"
        );
        trace!(response = %res.message.content, "ollama follow response");
        Ok(res.message.content)
    }
}

#[async_trait]
impl Chatter for OllamaProvider {
    async fn chat(&self, system_prompt: &str, history: &[Message]) -> Result<TextStream> {
        let mut prompt = system_prompt.to_string();
        for note in crate::types::take_prompt_context().await {
            prompt.push('\n');
            prompt.push_str(&note);
        }

        let mut msgs = Vec::with_capacity(history.len() + 1);
        msgs.push(ChatMessage::system(prompt.clone()));
        for m in history {
            let m = match m.role {
                Role::Assistant => ChatMessage::assistant(m.content.clone()),
                Role::User => ChatMessage::user(m.content.clone()),
            };
            msgs.push(m);
        }
        debug!(
            model = %self.model,
            history_len = history.len(),
            prompt_len = prompt.len(),
            "ollama chat"
        );
        debug!(
            model = %self.model,
            prompt = %prompt,
            history = ?history,
            "ollama chat request"
        );
        trace!(%prompt, ?history, "ollama chat request");
        let temperature = 0.8 + (rand::thread_rng().gen_range(-0.05..0.05));
        let options = ModelOptions::default().temperature(temperature);
        let req = ChatMessageRequest::new(self.model.clone(), msgs).options(options);
        let model = self.model.clone();
        let stream =
            self.client()
                .send_chat_messages_stream(req)
                .await?
                .map(move |res| match res {
                    Ok(r) => {
                        let chunk = r.message.content;
                        debug!(model = %model, chunk = %chunk, "ollama chat chunk");
                        trace!(%chunk, "ollama chat chunk");
                        Ok(chunk)
                    }
                    Err(e) => {
                        warn!(error = ?e, "ollama stream error");
                        Err(anyhow!("ollama stream error"))
                    }
                });
        Ok(Box::pin(stream))
    }
}

#[async_trait]
impl Vectorizer for OllamaProvider {
    /// Request text embeddings from Ollama.
    async fn vectorize(&self, text: &str) -> Result<Vec<f32>> {
        debug!(model = %self.model, len = text.len(), "ollama vectorize");
        trace!(?text, "ollama vectorize request");
        let mut attempts = 0;
        loop {
            attempts += 1;
            let req =
                GenerateEmbeddingsRequest::new(self.model.clone(), EmbeddingsInput::from(text));
            match timeout(
                Duration::from_secs(60),
                self.client().generate_embeddings(req),
            )
            .await
            {
                Err(_) => {
                    warn!("ollama vectorize timed out");
                    return Err(anyhow!("timeout"));
                }
                Ok(Ok(res)) => {
                    trace!(
                        embedding_len = res.embeddings.len(),
                        "ollama vectorize response"
                    );
                    let Some(embedding) = res.embeddings.into_iter().next() else {
                        warn!("ollama returned no embeddings");
                        return Err(anyhow!("empty embedding"));
                    };
                    return Ok(embedding);
                }
                Ok(Err(e)) => {
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
