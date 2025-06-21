use async_trait::async_trait;
use psyche::{Conversation, ErasedWit, Impression, Ling, Memory, Stimulus, Wit, WitAdapter};
use serde_json::Value;
use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as AsyncMutex;

#[derive(Default)]
struct RecMemory(AsyncMutex<Vec<String>>);

#[async_trait]
impl Memory for RecMemory {
    async fn store(&self, impression: &Impression<Value>) -> anyhow::Result<()> {
        self.0.lock().await.push(impression.summary.clone());
        Ok(())
    }
}

#[derive(Default)]
struct DummyWit {
    outputs: Mutex<Vec<Vec<Impression<()>>>>,
}

#[async_trait]
impl Wit<(), ()> for DummyWit {
    async fn observe(&self, _: ()) {}

    async fn tick(&self) -> Vec<Impression<()>> {
        self.outputs.lock().unwrap().pop().unwrap_or_default()
    }
}

async fn run_once(
    memory: Arc<dyn Memory>,
    wits: Vec<Arc<dyn ErasedWit + Send + Sync>>,
    ling: Arc<AsyncMutex<Ling>>,
) {
    let mut tasks = Vec::new();
    for wit in &wits {
        let wit = wit.clone();
        tasks.push(tokio::spawn(async move {
            let imps = wit.tick_erased().await;
            imps
        }));
    }
    let mut all = Vec::new();
    for t in tasks {
        if let Ok(imps) = t.await {
            all.extend(imps);
        }
    }
    if !all.is_empty() {
        let _ = memory.store_all(&all).await;
        ling.lock().await.add_impressions(&all).await;
    }
}

#[tokio::test]
async fn multiple_impressions_flow_to_memory_and_context() {
    let mem = Arc::new(RecMemory::default());
    let conversation = Arc::new(AsyncMutex::new(Conversation::default()));
    let ling = Arc::new(AsyncMutex::new(Ling::new("sys", conversation)));

    let wit = Arc::new(DummyWit {
        outputs: Mutex::new(vec![
            vec![Impression::new(
                vec![Stimulus::new(())],
                "c",
                None::<String>,
            )],
            vec![
                Impression::new(vec![Stimulus::new(())], "b", None::<String>),
                Impression::new(vec![Stimulus::new(())], "b2", None::<String>),
            ],
            vec![Impression::new(
                vec![Stimulus::new(())],
                "a",
                None::<String>,
            )],
            Vec::new(),
        ]),
    });
    let wits: Vec<Arc<dyn ErasedWit + Send + Sync>> = vec![Arc::new(WitAdapter::new(wit))];

    for _ in 0..4 {
        run_once(mem.clone(), wits.clone(), ling.clone()).await;
    }

    let stored = mem.0.lock().await.clone();
    assert_eq!(stored, vec!["a", "b", "b2", "c"]);
    let prompt = ling.lock().await.build_prompt().await;
    assert!(prompt.contains("a"));
    assert!(prompt.contains("b"));
    assert!(prompt.contains("b2"));
    assert!(prompt.contains("c"));
}
