//! Registry of available language model servers.
//!
//! The [`ModelRegistry`] type tracks [`ServerInfo`] entries which list the
//! supported models and attributes for each host.

use std::collections::HashMap;

use super::{Attribute, Capability};

/// Metadata describing a single model.
#[derive(Clone)]
pub struct ModelInfo {
    pub name: String,
    pub capabilities: Vec<Capability>,
    pub attributes: Vec<Attribute>,
}

/// Configuration for a running server and the models it provides.
#[derive(Clone)]
pub struct ServerInfo {
    pub address: String,
    pub models: HashMap<String, ModelInfo>,
    pub attributes: Vec<Attribute>,
}

/// Collection of known LLM servers.
pub struct ModelRegistry {
    servers: Vec<ServerInfo>,
}

impl ModelRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            servers: Vec::new(),
        }
    }

    /// Add a server definition.
    pub fn add_server(&mut self, server: ServerInfo) {
        self.servers.push(server);
    }

    /// Return all registered servers.
    pub fn servers(&self) -> &[ServerInfo] {
        &self.servers
    }
}

impl ServerInfo {
    /// Create a server entry with no models.
    pub fn new(address: impl Into<String>) -> Self {
        Self {
            address: address.into(),
            models: HashMap::new(),
            attributes: Vec::new(),
        }
    }

    /// Associate a model definition with this server.
    pub fn with_model(mut self, info: ModelInfo) -> Self {
        self.models.insert(info.name.clone(), info);
        self
    }
}

impl ModelInfo {
    pub fn new(
        name: impl Into<String>,
        capabilities: Vec<Capability>,
        attributes: Vec<Attribute>,
    ) -> Self {
        Self {
            name: name.into(),
            capabilities,
            attributes,
        }
    }
}
