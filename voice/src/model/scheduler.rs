//! Simple scheduler that selects a server by capability and optional attribute.

use super::{
    registry::{ModelRegistry, ServerInfo},
    Attribute, Capability,
};

/// Chooses which server to use for a given request.
pub struct ModelScheduler {
    registry: ModelRegistry,
}

impl ModelScheduler {
    /// Wrap an existing [`ModelRegistry`].
    pub fn new(registry: ModelRegistry) -> Self {
        Self { registry }
    }

    /// Find a server that supports the requested capability.
    pub fn select(&self, capability: Capability, prefer: Option<Attribute>) -> Option<&ServerInfo> {
        self.registry.servers().iter().find(|s| {
            s.models
                .values()
                .any(|m| m.capabilities.contains(&capability))
                && prefer.map_or(true, |attr| s.attributes.contains(&attr))
        })
    }
}
