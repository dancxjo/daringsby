use async_trait::async_trait;
use psyche::ling::Vectorizer;
use psyche::{BasicMemory, GraphStore, Impression, Memory, QdrantClient, Stimulus};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use tokio::time::{self, Duration};

#[derive(Default)]
struct HangingVec;

#[async_trait]
impl Vectorizer for HangingVec {
    async fn vectorize(&self, _text: &str) -> anyhow::Result<Vec<f32>> {
        futures::future::pending::<()>().await;
        Ok(Vec::new())
    }
}

#[derive(Default)]
struct DummyVec;

#[async_trait]
impl Vectorizer for DummyVec {
    async fn vectorize(&self, _text: &str) -> anyhow::Result<Vec<f32>> {
        Ok(vec![0.0])
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
async fn store_returns_after_timeout() {
    let neo = Arc::new(MockNeo4j::default());
    let mem = BasicMemory {
        vectorizer: Arc::new(HangingVec),
        qdrant: QdrantClient::default(),
        neo4j: neo.clone(),
    };

    let imp = Impression::new(vec![Stimulus::new(json!({"x":1}))], "hi", None::<String>);
    let fut = <dyn Memory>::store_serializable(&mem, &imp);
    let res = time::timeout(Duration::from_secs(6), fut).await;
    assert!(res.is_ok());
    assert_eq!(neo.0.lock().unwrap().len(), 1);
}

#[derive(Default)]
struct FailingNeo4j {
    log: Mutex<Vec<String>>,
    fail: Mutex<bool>,
}

#[async_trait]
impl GraphStore for FailingNeo4j {
    async fn store_data(&self, data: &Value) -> anyhow::Result<()> {
        let mut fail = self.fail.lock().unwrap();
        if *fail {
            *fail = false;
            anyhow::bail!("fail");
        }
        let json = serde_json::to_string(data)?;
        self.log.lock().unwrap().push(json);
        Ok(())
    }
}

#[tokio::test]
async fn store_all_continues_after_error() {
    let neo = Arc::new(FailingNeo4j {
        log: Mutex::new(Vec::new()),
        fail: Mutex::new(true),
    });
    let mem = BasicMemory {
        vectorizer: Arc::new(DummyVec),
        qdrant: QdrantClient::default(),
        neo4j: neo.clone(),
    };
    let imps = [
        Impression::new(vec![Stimulus::new(json!({"a":1}))], "a", None::<String>),
        Impression::new(vec![Stimulus::new(json!({"b":2}))], "b", None::<String>),
    ];
    mem.store_all(&imps).await.unwrap();
    assert_eq!(neo.log.lock().unwrap().len(), 1);
}
