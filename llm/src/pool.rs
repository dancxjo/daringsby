use std::pin::Pin;
use futures_core::Stream;

use crate::model::LLMServer;
use crate::traits::{LLMAttribute, LLMCapability, LLMError};
use crate::task::LinguisticTask;

pub struct LLMClientPool {
    servers: Vec<LLMServer>,
}

impl LLMClientPool {
    pub fn new() -> Self {
        Self { servers: Vec::new() }
    }

    pub fn add_server(&mut self, server: LLMServer) {
        self.servers.push(server);
    }

    fn find_server(&self, model: &str, attr: Option<LLMAttribute>) -> Option<&LLMServer> {
        self.servers.iter().find(|s| {
            s.models.contains_key(model) && attr.map_or(true, |a| s.attributes.contains(&a))
        })
    }

    pub fn model_capabilities(&self, model: &str) -> Option<Vec<LLMCapability>> {
        for server in &self.servers {
            if let Some(m) = server.models.get(model) {
                return Some(m.capabilities.clone());
            }
        }
        None
    }

    pub fn has_attribute(&self, model: &str, attr: LLMAttribute) -> bool {
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
        &self,
        task: &LinguisticTask,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String, LLMError>> + Send>>, LLMError> {
        let model = self
            .choose_model(&task.capabilities, task.prefer)
            .ok_or(LLMError::ModelNotFound)?;
        self.stream_chat(&model, &task.prompt).await
    }

    pub async fn stream_chat(
        &self,
        model: &str,
        prompt: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String, LLMError>> + Send>>, LLMError> {
        let server = self.find_server(model, None).ok_or(LLMError::ModelNotFound)?;
        server.client.stream_chat(model, prompt).await
    }
}
