use async_trait::async_trait;
use psyche::{
    Impression, Stimulus, Wit,
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
        self.0.lock().unwrap().push(impression.summary.clone());
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
            vec![Stimulus::new(format!("d{i}"))],
            format!("h{i}"),
            None::<String>,
        ))
        .await;
    }

    let out = wit.tick().await;
    let report = rx.recv().await.unwrap();
    assert_eq!(report.name, "Memory");
    assert!(report.output.contains("h0"));
    assert_eq!(out.len(), 1);
    assert!(out[0].stimuli[0].what.summary.contains("h1"));
    psyche::disable_debug("Memory").await;
}
