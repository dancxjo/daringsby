use async_trait::async_trait;
use psyche::{Impression, Memory, Stimulus};
use serde_json::Value;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct MockMemory(Arc<Mutex<Vec<String>>>);

#[async_trait]
impl Memory for MockMemory {
    async fn store(&self, impression: &Impression<Value>) -> anyhow::Result<()> {
        self.0.lock().unwrap().push(impression.summary.clone());
        Ok(())
    }
}

#[tokio::test]
async fn stores_impression() {
    let mem = MockMemory::default();
    <dyn Memory>::store_serializable(
        &mem,
        &Impression::new(vec![Stimulus::new(1)], "hello", None::<String>),
    )
    .await
    .unwrap();
    assert_eq!(mem.0.lock().unwrap().len(), 1);
}
