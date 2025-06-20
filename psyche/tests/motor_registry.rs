use async_trait::async_trait;
use psyche::motorcall::{Motor, MotorRegistry};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct RecMotor(Arc<Mutex<Vec<(HashMap<String, String>, String)>>>);

#[async_trait]
impl Motor for RecMotor {
    async fn execute(&self, attrs: HashMap<String, String>, content: String) {
        self.0.lock().unwrap().push((attrs, content));
    }
}

#[tokio::test]
async fn registry_invokes() {
    let motor = Arc::new(RecMotor::default());
    let mut reg = MotorRegistry::default();
    reg.register("test", motor.clone());
    let mut attrs = HashMap::new();
    attrs.insert("a".into(), "b".into());
    reg.invoke("test", attrs.clone(), "hi".into()).await;
    let calls = motor.0.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0.get("a").unwrap(), "b");
    assert_eq!(calls[0].1, "hi");
}
