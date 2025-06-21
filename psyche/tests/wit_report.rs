use async_trait::async_trait;
use chrono::Utc;
use psyche::ling::{Doer, Instruction};
use psyche::{Impression, Summarizer, Will, WitReport};
use tokio::sync::broadcast;
use uuid::Uuid;

#[derive(Clone)]
struct Dummy;

#[async_trait]
impl Doer for Dummy {
    async fn follow(&self, _: Instruction) -> anyhow::Result<String> {
        Ok("ok".to_string())
    }
}

#[tokio::test]
async fn will_sends_report() {
    let (tx, mut rx) = broadcast::channel(8);
    let will = Will::with_debug(Box::new(Dummy), tx.clone());
    // no report when disabled
    let _ = will
        .digest(&[Impression {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            headline: "".into(),
            details: None,
            raw_data: "go".to_string(),
        }])
        .await
        .unwrap();
    assert!(rx.try_recv().is_err());
    psyche::enable_debug("Will").await;
    let _ = will
        .digest(&[Impression {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            headline: "".into(),
            details: None,
            raw_data: "go".to_string(),
        }])
        .await
        .unwrap();
    let report = rx.recv().await.unwrap();
    assert_eq!(report.name, "Will");
    assert!(report.prompt.contains("go") || report.prompt.contains("Go"));
    assert_eq!(report.output, "ok");
    psyche::disable_debug("Will").await;
}
