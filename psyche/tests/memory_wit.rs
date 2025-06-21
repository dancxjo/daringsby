use async_trait::async_trait;
use psyche::{
    Impression, Wit, WitReport,
    wits::{Memory, MemoryWit},
};
use serde_json::Value;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

#[derive(Default)]
struct DummyMemory(Arc<Mutex<Vec<String>>>);

#[async_trait]
impl Memory for DummyMemory {
    async fn store(&self, impression: &Impression<Value>) -> anyhow::Result<()> {
        self.0.lock().unwrap().push(impression.headline.clone());
        Ok(())
    }
}

#[tokio::test]
async fn summarizes_and_emits_report() {
    let (tx, mut rx) = broadcast::channel(8);
    psyche::enable_debug("Memory").await;
    let mem = Arc::new(DummyMemory::default());
    let wit = MemoryWit::with_debug(mem.clone(), tx);

    for i in 0..5 {
        wit.observe(Impression::new(
            format!("h{i}"),
            None::<String>,
            format!("d{i}"),
        ))
        .await;
    }

    let out = wit.tick().await;
    let report = rx.recv().await.unwrap();
    assert_eq!(report.name, "Memory");
    assert!(report.output.contains("h0"));
    assert_eq!(out.len(), 1);
    assert!(out[0].raw_data.summary.contains("h1"));
    psyche::disable_debug("Memory").await;
}
