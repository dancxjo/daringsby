//! Language processing primitives including processors, providers and schedulers.
//! Processors implement chat completion, instruction following or embedding.
//! Providers manage processor instances, while a [`scheduler::Scheduler`]
//! distributes tasks across available providers.
use anyhow::Context;
use async_trait::async_trait;
use futures::stream::BoxStream;
use modeldb::{AiModel, ModelRepository};
use serde::{Deserialize, Serialize};

pub mod provider;
pub use provider::{ModelRunnerProvider, OllamaProvider, OpenAIProvider, ProviderProfile};

pub mod ollama_server;
pub mod profiling;
pub mod scheduler;
/// Role of a chat participant.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Role {
    System,
    User,
    Assistant,
}

/// A single chat message. Images may be attached.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
    #[serde(default)]
    pub images: Vec<Vec<u8>>, // raw image bytes
}

/// Task describing different operations a processor may handle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Task {
    ChatCompletion(ChatCompletionTask),
    SentenceEmbedding(SentenceEmbeddingTask),
    InstructionFollowing(InstructionFollowingTask),
}

/// Generate chat completions from a sequence of messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionTask {
    pub messages: Vec<Message>,
}

/// Produce embeddings for a single sentence. Images are currently only
/// supported for [`InstructionFollowingTask`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentenceEmbeddingTask {
    pub sentence: String,
}

/// Follow a natural language instruction and return textual output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstructionFollowingTask {
    pub instruction: String,
    #[serde(default)]
    pub images: Vec<Vec<u8>>,
}

/// Returned stream items from processors.
#[derive(Debug)]
pub enum TaskOutput {
    TextChunk(String),
    Embedding(Vec<f32>),
}

/// Capability describing supported task types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskKind {
    ChatCompletion,
    SentenceEmbedding,
    InstructionFollowing,
}

#[async_trait]
pub trait Processor {
    /// Advertise supported task types.
    fn capabilities(&self) -> Vec<TaskKind>;

    /// Process a task, producing a stream of results.
    async fn process(
        &self,
        task: Task,
    ) -> anyhow::Result<BoxStream<'static, anyhow::Result<TaskOutput>>>;
}

/// Ollama based implementation.
pub struct OllamaProcessor {
    client: ollama_rs::Ollama,
    pub model: String,
}

impl OllamaProcessor {
    /// Create a processor using the default Ollama client.
    pub fn new(model: &str) -> Self {
        Self::with_client(ollama_rs::Ollama::default(), model)
    }

    /// Create a processor backed by a custom Ollama client.
    ///
    /// ```no_run
    /// let client = ollama_rs::Ollama::new("http://localhost", 11434);
    /// let proc = lingproc::OllamaProcessor::with_client(client, "gemma3");
    /// assert_eq!(proc.model, "gemma3");
    /// ```
    pub fn with_client(client: ollama_rs::Ollama, model: &str) -> Self {
        Self {
            client,
            model: model.to_string(),
        }
    }

    async fn encode_sentence(&self, text: &str) -> Vec<f32> {
        text.split_whitespace()
            .map(|w| w.bytes().map(|b| b as f32).sum::<f32>())
            .collect()
    }
}

#[async_trait]
impl Processor for OllamaProcessor {
    fn capabilities(&self) -> Vec<TaskKind> {
        vec![
            TaskKind::ChatCompletion,
            TaskKind::InstructionFollowing,
            TaskKind::SentenceEmbedding,
        ]
    }

    async fn process(
        &self,
        task: Task,
    ) -> anyhow::Result<BoxStream<'static, anyhow::Result<TaskOutput>>> {
        match task {
            Task::ChatCompletion(c) => {
                use async_stream::stream;
                use futures::StreamExt;
                use ollama_rs::generation::completion::request::GenerationRequest;

                let prompt = c
                    .messages
                    .iter()
                    .map(|m| format!("{:?}: {}", m.role, m.content))
                    .collect::<Vec<_>>()
                    .join("\n");
                let req = GenerationRequest::new(self.model.clone(), prompt);
                let mut resp = self.client.generate_stream(req).await?;
                let s = stream! {
                    while let Some(chunk) = resp.next().await {
                        let chunk = chunk?;
                        for c in chunk {
                            yield Ok(TaskOutput::TextChunk(c.response));
                        }
                    }
                };
                Ok(Box::pin(s))
            }
            Task::InstructionFollowing(i) => {
                use async_stream::stream;
                use futures::StreamExt;
                use ollama_rs::generation::completion::request::GenerationRequest;

                let req = GenerationRequest::new(self.model.clone(), i.instruction);
                let mut resp = self.client.generate_stream(req).await?;
                let s = stream! {
                    while let Some(chunk) = resp.next().await {
                        let chunk = chunk?;
                        for c in chunk {
                            yield Ok(TaskOutput::TextChunk(c.response));
                        }
                    }
                };
                Ok(Box::pin(s))
            }
            Task::SentenceEmbedding(e) => {
                use async_stream::stream;
                let vec = self.encode_sentence(&e.sentence).await;
                let s = stream! {
                    yield Ok(TaskOutput::Embedding(vec));
                };
                Ok(Box::pin(s))
            }
        }
    }
}

/// OpenAI based implementation.
pub struct OpenAIProcessor {
    client: async_openai::Client<async_openai::config::OpenAIConfig>,
    pub model: String,
}

impl OpenAIProcessor {
    pub fn new(api_key: &str, model: &str) -> Self {
        let config = async_openai::config::OpenAIConfig::new().with_api_key(api_key);
        Self {
            client: async_openai::Client::with_config(config),
            model: model.to_string(),
        }
    }
}

#[async_trait]
impl Processor for OpenAIProcessor {
    fn capabilities(&self) -> Vec<TaskKind> {
        vec![TaskKind::ChatCompletion]
    }

    async fn process(
        &self,
        task: Task,
    ) -> anyhow::Result<BoxStream<'static, anyhow::Result<TaskOutput>>> {
        match task {
            Task::ChatCompletion(c) => {
                use async_openai::types::{
                    ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestMessage,
                    ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
                    CreateChatCompletionRequestArgs,
                };
                use async_stream::stream;
                use futures::StreamExt;

                let msgs: Vec<ChatCompletionRequestMessage> = c
                    .messages
                    .iter()
                    .map(|m| match m.role {
                        Role::System => ChatCompletionRequestSystemMessageArgs::default()
                            .content(m.content.clone())
                            .build()
                            .unwrap()
                            .into(),
                        Role::Assistant => ChatCompletionRequestAssistantMessageArgs::default()
                            .content(m.content.clone())
                            .build()
                            .unwrap()
                            .into(),
                        Role::User => ChatCompletionRequestUserMessageArgs::default()
                            .content(m.content.clone())
                            .build()
                            .unwrap()
                            .into(),
                    })
                    .collect();

                let req = CreateChatCompletionRequestArgs::default()
                    .model(&self.model)
                    .messages(msgs)
                    .stream(true)
                    .build()?;
                let mut resp = self.client.chat().create_stream(req).await?;
                let s = stream! {
                    while let Some(chunk) = resp.next().await {
                        let chunk = chunk?;
                        if let Some(c) = chunk.choices.first() {
                            if let Some(ref content) = c.delta.content {
                                yield Ok(TaskOutput::TextChunk(content.clone()));
                            }
                        }
                    }
                };
                Ok(Box::pin(s))
            }
            _ => Err(anyhow::anyhow!("task not supported")),
        }
    }
}

/// Ensure a model exists on the local Ollama server, pulling it if necessary.
///
/// ```no_run
/// # tokio_test::block_on(async {
/// lingproc::ensure_model_available("gemma3").await.unwrap();
/// # });
/// ```
pub async fn ensure_model_available(model: &str) -> anyhow::Result<()> {
    ensure_model_with_client(&ollama_rs::Ollama::default(), model).await
}

async fn ensure_model_with_client(client: &ollama_rs::Ollama, model: &str) -> anyhow::Result<()> {
    use futures::StreamExt;
    let models = client.list_local_models().await.unwrap_or_default();
    if models.iter().any(|m| m.name == model) {
        return Ok(());
    }
    let mut stream = client
        .pull_model_stream(model.to_string(), false)
        .await
        .with_context(|| {
            format!(
                "failed to connect to Ollama server while pulling model `{model}`. Are the servers running?"
            )
        })?;
    let pb = indicatif::ProgressBar::new_spinner();
    pb.enable_steady_tick(std::time::Duration::from_millis(100));
    while let Some(status) = stream.next().await {
        let status = status?;
        if let (Some(total), Some(completed)) = (status.total, status.completed) {
            pb.set_length(total);
            pb.set_position(completed);
        }
        pb.set_message(status.message);
    }
    pb.finish_with_message("model ready");
    Ok(())
}

/// Default model repository used by examples and tests.
pub fn default_repository() -> ModelRepository {
    let mut repo = modeldb::ollama_models();
    repo.add_model(AiModel {
        name: "gpt4".into(),
        supports_images: true,
        speed: Some(1.0),
        cost_per_token: Some(0.03),
        capabilities: vec![
            modeldb::Capability::ChatCompletion,
            modeldb::Capability::InstructionFollowing,
        ],
    });
    repo
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;

    struct EchoProcessor;

    #[async_trait]
    impl Processor for EchoProcessor {
        fn capabilities(&self) -> Vec<TaskKind> {
            vec![TaskKind::InstructionFollowing]
        }

        async fn process(
            &self,
            task: Task,
        ) -> anyhow::Result<BoxStream<'static, anyhow::Result<TaskOutput>>> {
            match task {
                Task::InstructionFollowing(t) => {
                    use async_stream::stream;
                    let instr = t.instruction.clone();
                    let s = stream! {
                        yield Ok(TaskOutput::TextChunk(instr));
                    };
                    Ok(Box::pin(s))
                }
                _ => Err(anyhow::anyhow!("task not supported")),
            }
        }
    }

    #[tokio::test]
    async fn echo_instruction() {
        let proc = EchoProcessor;
        let task = Task::InstructionFollowing(InstructionFollowingTask {
            instruction: "ping".into(),
            images: vec![],
        });
        let mut stream = proc.process(task).await.unwrap();
        let first = stream.next().await.unwrap().unwrap();
        match first {
            TaskOutput::TextChunk(t) => assert_eq!(t, "ping"),
            _ => panic!("wrong output"),
        }
    }

    #[tokio::test]
    async fn repo_has_models() {
        let repo = default_repository();
        assert!(repo.find("gpt4").is_some());
        assert!(repo.find("gemma3").is_some());
    }

    #[tokio::test]
    async fn embedding_task_returns_vector() {
        let proc = OllamaProcessor::new("nomic-embed-text");
        let task = Task::SentenceEmbedding(SentenceEmbeddingTask {
            sentence: "hello world".into(),
        });
        let mut stream = proc.process(task).await.unwrap();
        let first = stream.next().await.unwrap().unwrap();
        match first {
            TaskOutput::Embedding(v) => assert!(!v.is_empty()),
            _ => panic!("wrong output"),
        }
    }
    #[tokio::test]
    async fn profiler_records_time() {
        use crate::profiling::ProfilingProcessor;
        use std::time::Duration;
        let proc = ProfilingProcessor::new(EchoProcessor);
        let task = Task::InstructionFollowing(InstructionFollowingTask {
            instruction: "pong".into(),
            images: vec![],
        });
        let mut stream = proc.process(task).await.unwrap();
        while let Some(_c) = stream.next().await {}
        assert!(stream.next().await.is_none());
        let d = proc.durations();
        assert_eq!(d.len(), 1);
        assert!(d[0] > Duration::from_secs(0));
    }

    #[tokio::test]
    async fn pulls_if_missing() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        use warp::Filter;

        let pulled = Arc::new(AtomicUsize::new(0));
        let pulled_c = pulled.clone();
        let tags = warp::path("api")
            .and(warp::path("tags"))
            .map(|| warp::reply::json(&serde_json::json!({ "models": [] })));
        let pull = warp::path("api").and(warp::path("pull")).map(move || {
            pulled_c.fetch_add(1, Ordering::SeqCst);
            warp::reply::json(&serde_json::json!({"status":"ok"}))
        });
        let routes = tags.or(pull);
        let (addr, server) = warp::serve(routes).bind_ephemeral(([127, 0, 0, 1], 0));
        tokio::task::spawn(server);

        let client = ollama_rs::Ollama::new(format!("http://{}", addr.ip()), addr.port());
        super::ensure_model_with_client(&client, "missing")
            .await
            .unwrap();
        assert_eq!(pulled.load(Ordering::SeqCst), 1);
    }
}
