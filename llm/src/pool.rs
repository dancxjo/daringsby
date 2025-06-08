//! Connection pool and lightweight scheduler for language model servers.
//!
//! [`LLMClientPool`] keeps track of multiple [`LLMServer`] instances and records
//! simple latency statistics. It can then route [`LinguisticTask`] requests to the best
//! available model.

use futures_core::Stream;
use futures_util::ready;
use tokio::sync::Semaphore;
use tokio::sync::OwnedSemaphorePermit;
use std::task::{Context, Poll};
use std::{
    pin::Pin,
    sync::{Arc, Mutex},
    time::Instant,
};

use crate::client::OllamaClient;
use crate::model::{LLMModel, LLMServer};
use crate::task::LinguisticTask;
use crate::traits::{LLMAttribute, LLMCapability, LLMError};

/// Pool of language model servers with basic latency profiling.
pub struct LLMClientPool {
    servers: Vec<LLMServer>,
    profiles: Vec<Arc<Mutex<ServerProfile>>>,
    locks: Vec<Arc<Semaphore>>,
}

/// Moving average of latency samples for a server.
#[derive(Default)]
struct ServerProfile {
    latency_ms: f64,
    samples: u32,
}

impl ServerProfile {
    fn record(&mut self, sample: f64) {
        self.samples += 1;
        if self.samples == 1 {
            self.latency_ms = sample;
        } else {
            let n = self.samples as f64;
            self.latency_ms = ((n - 1.0) / n) * self.latency_ms + (sample / n);
        }
    }
}

impl LLMClientPool {
    pub fn new() -> Self {
        Self {
            servers: Vec::new(),
            profiles: Vec::new(),
            locks: Vec::new(),
        }
    }

    pub fn add_server(&mut self, server: LLMServer) {
        self.servers.push(server);
        self.profiles
            .push(Arc::new(Mutex::new(ServerProfile::default())));
        self.locks.push(Arc::new(Semaphore::new(1)));
    }

    pub fn add_ollama_host(
        &mut self,
        url: impl AsRef<str>,
        models: Vec<LLMModel>,
        attrs: Vec<LLMAttribute>,
    ) {
        let client = Arc::new(OllamaClient::new(url.as_ref()));
        let mut server = LLMServer::new(client);
        for attr in attrs {
            server = server.with_attribute(attr);
        }
        for model in models {
            server = server.with_model(model);
        }
        self.add_server(server);
    }

    fn find_server(
        &mut self,
        model: &str,
        attr: Option<LLMAttribute>,
    ) -> Option<(usize, &LLMServer)> {
        let mut matching: Vec<_> = self
            .servers
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                s.models.contains_key(model) && attr.map_or(true, |a| s.attributes.contains(&a))
            })
            .collect();
        if matching.is_empty() {
            return None;
        }
        matching.sort_by(|a, b| {
            let la = self.profiles[a.0].lock().unwrap().latency_ms;
            let lb = self.profiles[b.0].lock().unwrap().latency_ms;
            la.partial_cmp(&lb).unwrap_or(std::cmp::Ordering::Equal)
        });
        let idx = matching[0].0;
        Some((idx, &self.servers[idx]))
    }

    pub fn model_capabilities(&self, model: &str) -> Option<Vec<LLMCapability>> {
        for server in &self.servers {
            if let Some(m) = server.models.get(model) {
                return Some(m.capabilities.clone());
            }
        }
        None
    }

    pub fn has_attribute(&mut self, model: &str, attr: LLMAttribute) -> bool {
        self.find_server(model, Some(attr)).is_some()
    }

    pub fn choose_model(
        &self,
        caps: &[LLMCapability],
        prefer: Option<LLMAttribute>,
    ) -> Option<String> {
        for server in &self.servers {
            if prefer.map_or(true, |a| server.attributes.contains(&a)) {
                for (name, model) in &server.models {
                    if caps.iter().all(|c| model.capabilities.contains(c)) {
                        return Some(name.clone());
                    }
                }
            }
        }
        None
    }

    pub async fn run_task(
        &mut self,
        task: &LinguisticTask,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String, LLMError>> + Send>>, LLMError> {
        let model = self
            .choose_model(&task.capabilities, task.prefer)
            .ok_or(LLMError::ModelNotFound)?;
        self.stream_chat_internal(&model, &task.prompt, task.droppable).await
    }

    pub async fn stream_chat(
        &mut self,
        model: &str,
        prompt: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String, LLMError>> + Send>>, LLMError> {
        self.stream_chat_internal(model, prompt, false).await
    }

    async fn stream_chat_internal(
        &mut self,
        model: &str,
        prompt: &str,
        droppable: bool,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String, LLMError>> + Send>>, LLMError> {
        let idx = self
            .find_server(model, None)
            .map(|(i, _)| i)
            .ok_or(LLMError::ModelNotFound)?;
        let client = Arc::clone(&self.servers[idx].client);
        let sem = Arc::clone(&self.locks[idx]);
        let permit = if droppable {
            sem.try_acquire_owned().map_err(|_| LLMError::QueueFull)?
        } else {
            sem.acquire_owned().await.map_err(|_| LLMError::QueueFull)?
        };
        let start = Instant::now();
        let stream = match client.stream_chat(model, prompt).await {
            Ok(s) => s,
            Err(e) => return Err(e),
        };
        let profile = Arc::clone(&self.profiles[idx]);
        let timed = ProfilingStream {
            inner: stream,
            start,
            recorded: false,
            profile,
            _permit: permit,
        };
        Ok(Box::pin(timed))
    }
}

struct ProfilingStream<S> {
    inner: S,
    start: Instant,
    recorded: bool,
    profile: Arc<Mutex<ServerProfile>>,
    _permit: OwnedSemaphorePermit,
}

impl<S: Stream<Item = Result<String, LLMError>> + Unpin> Stream for ProfilingStream<S> {
    type Item = Result<String, LLMError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let item = ready!(Pin::new(&mut self.inner).poll_next(cx));
        if let Some(res) = item {
            if !self.recorded {
                let elapsed = self.start.elapsed().as_millis() as f64;
                if let Ok(mut p) = self.profile.lock() {
                    p.record(elapsed);
                }
                self.recorded = true;
            }
            Poll::Ready(Some(res))
        } else {
            Poll::Ready(None)
        }
    }
}
