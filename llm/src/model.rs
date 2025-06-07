//! Data structures describing available language models and servers.
//!
//! A [`LLMServer`] wraps a concrete [`LLMClient`]
//! implementation and the set of [`LLMModel`]s it exposes. These types are used
//! by the scheduler in [`crate::pool`] to select an appropriate model for a
//! [`crate::task::LinguisticTask`].

use std::collections::HashMap;
use std::sync::Arc;

use crate::traits::{LLMAttribute, LLMCapability, LLMClient};

#[derive(Clone)]
pub struct LLMModel {
    pub name: String,
    pub capabilities: Vec<LLMCapability>,
}

#[derive(Clone)]
pub struct LLMServer {
    pub client: Arc<dyn LLMClient>,
    pub models: HashMap<String, LLMModel>,
    pub attributes: Vec<LLMAttribute>,
}

impl LLMServer {
    pub fn new(client: Arc<dyn LLMClient>) -> Self {
        Self {
            client,
            models: HashMap::new(),
            attributes: Vec::new(),
        }
    }

    pub fn with_attribute(mut self, attr: LLMAttribute) -> Self {
        self.attributes.push(attr);
        self
    }

    pub fn with_model(mut self, model: LLMModel) -> Self {
        self.models.insert(model.name.clone(), model);
        self
    }
}

impl LLMModel {
    pub fn new(name: impl Into<String>, capabilities: Vec<LLMCapability>) -> Self {
        Self {
            name: name.into(),
            capabilities,
        }
    }
}
