use std::pin::Pin;
use futures_core::Stream;

use crate::model::LLMServer;
use crate::traits::{LLMAttribute, LLMCapability, LLMError};

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

    pub async fn stream_chat(
        &self,
        model: &str,
        prompt: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String, LLMError>> + Send>>, LLMError> {
        let server = self.find_server(model, None).ok_or(LLMError::ModelNotFound)?;
        server.client.stream_chat(model, prompt).await
    }
}
