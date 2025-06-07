use std::{pin::Pin, collections::HashMap};
use futures_core::Stream;

use crate::model::{LLMServer, LLMModel};
use crate::client::OllamaClient;
use std::sync::Arc;
use crate::traits::{LLMAttribute, LLMCapability, LLMError};
use crate::task::LinguisticTask;

pub struct LLMClientPool {
    servers: Vec<LLMServer>,
    next: HashMap<String, usize>,
}

impl LLMClientPool {
    pub fn new() -> Self {
        Self { servers: Vec::new(), next: HashMap::new() }
    }

    pub fn add_server(&mut self, server: LLMServer) {
        self.servers.push(server);
    }

    /// Add an Ollama host with the provided models and attributes.
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

    fn find_server(&mut self, model: &str, attr: Option<LLMAttribute>) -> Option<&LLMServer> {
        let matching: Vec<_> = self
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
        let idx = self.next.entry(model.to_string()).or_insert(0);
        let server = &self.servers[matching[*idx % matching.len()].0];
        *idx += 1;
        Some(server)
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

    /// Choose a model that satisfies all required capabilities and optional attribute.
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

    /// Execute a [`LinguisticTask`] by selecting an appropriate model.
    pub async fn run_task(
        &mut self,
        task: &LinguisticTask,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String, LLMError>> + Send>>, LLMError> {
        let model = self
            .choose_model(&task.capabilities, task.prefer)
            .ok_or(LLMError::ModelNotFound)?;
        self.stream_chat(&model, &task.prompt).await
    }

    pub async fn stream_chat(
        &mut self,
        model: &str,
        prompt: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String, LLMError>> + Send>>, LLMError> {
        let server = self.find_server(model, None).ok_or(LLMError::ModelNotFound)?;
        server.client.stream_chat(model, prompt).await
    }
}
