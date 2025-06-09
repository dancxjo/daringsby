use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use crate::{Processor, Task, TaskKind, TaskOutput, profiling::ProfilingProcessor};
use futures::StreamExt;
use futures::stream::BoxStream;
use modeldb::AiModel;

/// Aggregated profile information for a provider.
#[derive(Default, Debug, Clone)]
pub struct ProviderProfile {
    runs: HashMap<String, usize>,
}

impl ProviderProfile {
    pub(crate) fn record(&mut self, model: &str) {
        *self.runs.entry(model.to_string()).or_insert(0) += 1;
    }

    /// Number of times `model` has been run.
    pub fn runs(&self, model: &str) -> usize {
        self.runs.get(model).copied().unwrap_or(0)
    }
}

/// Abstraction over something that can provide processors for various models.
#[async_trait]
pub trait ModelRunnerProvider {
    /// Retrieve models this provider can run.
    async fn models(&self) -> anyhow::Result<Vec<AiModel>>;

    /// Obtain a processor for a given model. Implementations should track
    /// profiling data and active process counts.
    async fn processor_for(&self, model: &str) -> anyhow::Result<Box<dyn Processor + Send + Sync>>;

    /// Current number of active processes spawned by this provider.
    fn active(&self) -> usize;

    /// Lightweight heartbeat check.
    async fn heartbeat(&self) -> bool;

    /// More thorough health check.
    async fn health_check(&self) -> bool;

    /// Aggregated profile data.
    fn profile(&self) -> ProviderProfile;
}

/// Internal helper used by providers to wrap processors with profiling and
/// active process accounting.
pub(crate) struct ManagedProcessor<P> {
    inner: P,
    profile: Arc<Mutex<ProviderProfile>>,
    active: Arc<Mutex<usize>>,
    model: String,
}

impl<P> ManagedProcessor<P> {
    fn new(
        inner: P,
        profile: Arc<Mutex<ProviderProfile>>,
        active: Arc<Mutex<usize>>,
        model: String,
    ) -> Self {
        *active.lock().unwrap() += 1;
        Self {
            inner,
            profile,
            active,
            model,
        }
    }
}

#[async_trait]
impl<P: Processor + Send + Sync + 'static> Processor for ManagedProcessor<ProfilingProcessor<P>> {
    fn capabilities(&self) -> Vec<TaskKind> {
        self.inner.capabilities()
    }

    async fn process(
        &self,
        task: Task,
    ) -> anyhow::Result<BoxStream<'static, anyhow::Result<TaskOutput>>> {
        let profile = self.profile.clone();
        let active = self.active.clone();
        let model = self.model.clone();
        let mut inner_stream = self.inner.process(task).await?;
        let s = async_stream::stream! {
            while let Some(item) = inner_stream.next().await {
                yield item;
            }
            profile.lock().unwrap().record(&model);
            *active.lock().unwrap() -= 1;
        };
        Ok(Box::pin(s))
    }
}

/// Simple provider implementation for an Ollama server.
pub struct OllamaProvider {
    models: Vec<AiModel>,
    profile: Arc<Mutex<ProviderProfile>>,
    active: Arc<Mutex<usize>>,
}

impl OllamaProvider {
    /// Create a new provider with the given models.
    pub fn new(models: Vec<AiModel>) -> Self {
        Self {
            models,
            profile: Arc::new(Mutex::new(ProviderProfile::default())),
            active: Arc::new(Mutex::new(0)),
        }
    }
}

#[async_trait]
impl ModelRunnerProvider for OllamaProvider {
    async fn models(&self) -> anyhow::Result<Vec<AiModel>> {
        Ok(self.models.clone())
    }

    async fn processor_for(&self, model: &str) -> anyhow::Result<Box<dyn Processor + Send + Sync>> {
        crate::ensure_model_available(model).await?;
        let proc = crate::OllamaProcessor::new(model);
        let proc = ProfilingProcessor::new(proc);
        Ok(Box::new(ManagedProcessor::new(
            proc,
            self.profile.clone(),
            self.active.clone(),
            model.to_string(),
        )))
    }

    fn active(&self) -> usize {
        *self.active.lock().unwrap()
    }

    async fn heartbeat(&self) -> bool {
        true
    }

    async fn health_check(&self) -> bool {
        true
    }

    fn profile(&self) -> ProviderProfile {
        self.profile.lock().unwrap().clone()
    }
}

/// Simple provider implementation for an OpenAI subscription.
pub struct OpenAIProvider {
    api_key: String,
    models: Vec<AiModel>,
    profile: Arc<Mutex<ProviderProfile>>,
    active: Arc<Mutex<usize>>,
}

impl OpenAIProvider {
    pub fn new(api_key: &str, models: Vec<AiModel>) -> Self {
        Self {
            api_key: api_key.into(),
            models,
            profile: Arc::new(Mutex::new(ProviderProfile::default())),
            active: Arc::new(Mutex::new(0)),
        }
    }
}

#[async_trait]
impl ModelRunnerProvider for OpenAIProvider {
    async fn models(&self) -> anyhow::Result<Vec<AiModel>> {
        Ok(self.models.clone())
    }

    async fn processor_for(&self, model: &str) -> anyhow::Result<Box<dyn Processor + Send + Sync>> {
        let proc = crate::OpenAIProcessor::new(&self.api_key, model);
        let proc = ProfilingProcessor::new(proc);
        Ok(Box::new(ManagedProcessor::new(
            proc,
            self.profile.clone(),
            self.active.clone(),
            model.to_string(),
        )))
    }

    fn active(&self) -> usize {
        *self.active.lock().unwrap()
    }

    async fn heartbeat(&self) -> bool {
        true
    }

    async fn health_check(&self) -> bool {
        true
    }

    fn profile(&self) -> ProviderProfile {
        self.profile.lock().unwrap().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;

    struct Echo;

    #[async_trait]
    impl Processor for Echo {
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
                    let s = stream! { yield Ok(TaskOutput::TextChunk(t.instruction)); };
                    Ok(Box::pin(s))
                }
                _ => Err(anyhow::anyhow!("unsupported")),
            }
        }
    }

    struct DummyProvider {
        profile: Arc<Mutex<ProviderProfile>>,
        active: Arc<Mutex<usize>>,
    }

    impl DummyProvider {
        fn new() -> Self {
            Self {
                profile: Arc::new(Mutex::new(ProviderProfile::default())),
                active: Arc::new(Mutex::new(0)),
            }
        }
    }

    #[async_trait]
    impl ModelRunnerProvider for DummyProvider {
        async fn models(&self) -> anyhow::Result<Vec<AiModel>> {
            Ok(Vec::new())
        }

        async fn processor_for(
            &self,
            _model: &str,
        ) -> anyhow::Result<Box<dyn Processor + Send + Sync>> {
            let proc = ProfilingProcessor::new(Echo);
            Ok(Box::new(ManagedProcessor::new(
                proc,
                self.profile.clone(),
                self.active.clone(),
                "echo".into(),
            )))
        }

        fn active(&self) -> usize {
            *self.active.lock().unwrap()
        }

        async fn heartbeat(&self) -> bool {
            true
        }

        async fn health_check(&self) -> bool {
            true
        }

        fn profile(&self) -> ProviderProfile {
            self.profile.lock().unwrap().clone()
        }
    }

    #[tokio::test]
    async fn profile_tracks_runs() {
        let provider = DummyProvider::new();
        let proc = provider.processor_for("any").await.unwrap();
        assert_eq!(provider.active(), 1);
        let task = Task::InstructionFollowing(crate::InstructionFollowingTask {
            instruction: "hi".into(),
            images: vec![],
        });
        let mut s = proc.process(task).await.unwrap();
        while let Some(_c) = s.next().await {}
        assert!(s.next().await.is_none());
        assert_eq!(provider.profile().runs("echo"), 1);
        assert_eq!(provider.active(), 0);
    }
}
