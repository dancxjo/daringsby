use super::{registry::{ModelRegistry, ServerInfo}, Capability, Attribute};

pub struct ModelScheduler {
    registry: ModelRegistry,
}

impl ModelScheduler {
    pub fn new(registry: ModelRegistry) -> Self { Self { registry } }

    pub fn select(&self, capability: Capability, prefer: Option<Attribute>) -> Option<&ServerInfo> {
        self.registry.servers().iter().find(|s| {
            s.models.values().any(|m| m.capabilities.contains(&capability))
                && prefer.map_or(true, |attr| s.attributes.contains(&attr))
        })
    }
}
