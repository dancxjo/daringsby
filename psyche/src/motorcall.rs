use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::info;

#[async_trait]
pub trait Motor: Send + Sync {
    async fn execute(&self, attrs: HashMap<String, String>, content: String);
}

#[derive(Clone, Default)]
pub struct MotorRegistry {
    motors: HashMap<String, Arc<dyn Motor>>,
}

impl MotorRegistry {
    pub fn register(&mut self, name: &str, motor: Arc<dyn Motor>) {
        self.motors.insert(name.to_string(), motor);
    }

    pub async fn invoke(&self, name: &str, attrs: HashMap<String, String>, content: String) {
        if let Some(m) = self.motors.get(name) {
            info!(target: "motor", %name, ?attrs, %content, "invoking motor");
            m.execute(attrs, content).await;
        } else {
            info!(target: "motor", %name, "motor not found");
        }
    }
}
