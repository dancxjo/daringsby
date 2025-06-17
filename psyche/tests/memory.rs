use async_trait::async_trait;
use psyche::{Impression, Memory};
use serde_json::Value;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct MockMemory(Arc<Mutex<Vec<String>>>);

#[async_trait]
impl Memory for MockMemory {
    async fn store(&self, impression: &Impression<Value>) -> anyhow::Result<()> {
        self.0.lock().unwrap().push(impression.headline.clone());
        Ok(())
    }
}

#[tokio::test]
async fn stores_impression() {
    let mem = MockMemory::default();
    <dyn Memory>::store_serializable(&mem, &Impression::new("hello", None::<String>, 1))
        .await
        .unwrap();
    assert_eq!(mem.0.lock().unwrap().len(), 1);
}
