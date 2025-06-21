use async_trait::async_trait;
use psyche::ling::Vectorizer;
use psyche::{BasicMemory, GraphStore, Impression, Memory, QdrantClient, Stimulus};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct DummyVec;

#[async_trait]
impl Vectorizer for DummyVec {
    async fn vectorize(&self, _text: &str) -> anyhow::Result<Vec<f32>> {
        Ok(vec![0.1])
    }
}

#[derive(Default)]
struct MockNeo4j(Mutex<Vec<String>>);

#[async_trait]
impl GraphStore for MockNeo4j {
    async fn store_data(&self, data: &Value) -> anyhow::Result<()> {
        let json = serde_json::to_string(data)?;
        self.0.lock().unwrap().push(json);
        Ok(())
    }
}

#[tokio::test]
async fn memory_logs_cypher() {
    let store = Arc::new(MockNeo4j::default());
    let mem = BasicMemory {
        vectorizer: Arc::new(DummyVec),
        qdrant: QdrantClient::default(),
        neo4j: store.clone(),
    };

    <dyn Memory>::store_serializable(
        &mem,
        &Impression::new(vec![Stimulus::new(json!({"x":1}))], "hello", None::<String>),
    )
    .await
    .unwrap();

    let logs = store.0.lock().unwrap().clone();
    assert_eq!(logs.len(), 1);
    assert!(logs[0].contains("\"x\":1"));
}
