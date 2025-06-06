use std::collections::HashMap;

use super::{Attribute, Capability};

#[derive(Clone)]
pub struct ModelInfo {
    pub name: String,
    pub capabilities: Vec<Capability>,
    pub attributes: Vec<Attribute>,
}

#[derive(Clone)]
pub struct ServerInfo {
    pub address: String,
    pub models: HashMap<String, ModelInfo>,
    pub attributes: Vec<Attribute>,
}

pub struct ModelRegistry {
    servers: Vec<ServerInfo>,
}

impl ModelRegistry {
    pub fn new() -> Self { Self { servers: Vec::new() } }

    pub fn add_server(&mut self, server: ServerInfo) { self.servers.push(server); }

    pub fn servers(&self) -> &[ServerInfo] { &self.servers }
}

impl ServerInfo {
    pub fn new(address: impl Into<String>) -> Self {
        Self { address: address.into(), models: HashMap::new(), attributes: Vec::new() }
    }

    pub fn with_model(mut self, info: ModelInfo) -> Self {
        self.models.insert(info.name.clone(), info);
        self
    }
}

impl ModelInfo {
    pub fn new(name: impl Into<String>, capabilities: Vec<Capability>, attributes: Vec<Attribute>) -> Self {
        Self { name: name.into(), capabilities, attributes }
    }
}
