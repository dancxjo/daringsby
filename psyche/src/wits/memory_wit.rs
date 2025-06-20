use crate::wit::Wit;
use crate::{Impression, wits::Memory};
use async_trait::async_trait;
use serde_json::Value;
use std::sync::{Arc, Mutex};
use tracing::debug;

/// Wit that persists impressions into the configured [`Memory`].
pub struct MemoryWit {
    memory: Arc<dyn Memory>,
    buffer: Mutex<Vec<Impression<Value>>>,
}

impl MemoryWit {
    /// Create a new `MemoryWit` using the given storage backend.
    pub fn new(memory: Arc<dyn Memory>) -> Self {
        Self {
            memory,
            buffer: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl Wit<Impression<Value>, ()> for MemoryWit {
    async fn observe(&self, input: Impression<Value>) {
        self.buffer.lock().unwrap().push(input);
    }

    async fn tick(&self) -> Vec<Impression<()>> {
        let items = {
            let mut buf = self.buffer.lock().unwrap();
            if buf.is_empty() {
                return Vec::new();
            }
            let data = buf.drain(..).collect::<Vec<_>>();
            data
        };
        for imp in items {
            debug!("memory storing impression: {}", imp.headline);
            let _ = self.memory.store(&imp).await;
        }
        Vec::new()
    }
}
