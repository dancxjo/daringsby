use async_trait::async_trait;
use lingproc::Vectorizer;
use psyche::{
    BasicMemory, GraphStore, ImageData, Impression, Memory, QdrantClient, Stimulus,
    image_content_id,
};
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
struct FailingVec;

#[async_trait]
impl Vectorizer for FailingVec {
    async fn vectorize(&self, _text: &str) -> anyhow::Result<Vec<f32>> {
        anyhow::bail!("skip qdrant")
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
    assert!(logs[0].contains("\"op\":\"merge_graph\""));
    assert!(logs[0].contains("\"label\":\"Sensation\""));
    assert!(logs[0].contains("\"kind\":\"cognitive\""));
    assert!(logs[0].contains("\"how\":\"hello\""));
    assert!(!logs[0].contains("\"label\":\"Impression\""));
    assert!(!logs[0].contains("\"label\":\"Stimulus\""));
    assert!(!logs[0].contains("\"type\":\"HAS_STIMULUS\""));
    assert!(logs[0].contains("\\\"x\\\":1"));
}

#[tokio::test]
async fn memory_graph_stores_reconstructable_image_payload() {
    let store = Arc::new(MockNeo4j::default());
    let mem = BasicMemory {
        vectorizer: Arc::new(FailingVec),
        qdrant: QdrantClient::default(),
        neo4j: store.clone(),
    };
    let image = ImageData {
        mime: "image/png".into(),
        base64: "abc123".into(),
        captured_at: Some("2026-05-05T12:34:56Z".into()),
    };
    let image_id = image_content_id(&image);

    <dyn Memory>::store_serializable(
        &mem,
        &Impression::new(vec![Stimulus::new(image)], "image", None::<String>),
    )
    .await
    .unwrap();

    let logs = store.0.lock().unwrap().clone();
    assert_eq!(logs.len(), 1);
    assert!(logs[0].contains(&format!("\"id\":\"{image_id}\"")));
    assert!(logs[0].contains("\"mime\":\"image/png\""));
    assert!(logs[0].contains("\"base64\":\"abc123\""));
    assert!(logs[0].contains("\\\"base64\\\":\\\"abc123\\\""));
}

#[tokio::test]
async fn memory_graph_links_cognitive_sensation_to_source_sensation() {
    let store = Arc::new(MockNeo4j::default());
    let mem = BasicMemory {
        vectorizer: Arc::new(FailingVec),
        qdrant: QdrantClient::default(),
        neo4j: store.clone(),
    };

    <dyn Memory>::store_serializable(
        &mem,
        &Impression::new(
            vec![Stimulus::with_source_sensation_ids(
                "heard hi",
                chrono::Utc::now(),
                ["sensation:utterance:1"],
            )],
            "greeting",
            None::<String>,
        ),
    )
    .await
    .unwrap();

    let logs = store.0.lock().unwrap().clone();
    assert_eq!(logs.len(), 1);
    assert!(logs[0].contains("\"source_sensation_ids\":[\"sensation:utterance:1\"]"));
    assert!(logs[0].contains("\"label\":\"SourceSensationRef\",\"id\":\"sensation:utterance:1\""));
    assert!(logs[0].contains("\"type\":\"DERIVED_FROM\""));
    assert!(!logs[0].contains("\"label\":\"Impression\""));
    assert!(!logs[0].contains("\"label\":\"Stimulus\""));
    assert!(!logs[0].contains("\"type\":\"HAS_STIMULUS\""));
}
