use async_trait::async_trait;
use lingproc::Instruction as LlmInstruction;
use psyche::traits::Doer;
use psyche::wits::Will;
use psyche::{Impression, Stimulus, TopicBus, Wit};
use std::sync::Arc;
use tokio::sync::broadcast;

#[derive(Clone)]
struct Dummy;

#[async_trait]
impl Doer for Dummy {
    async fn follow(&self, _: LlmInstruction) -> anyhow::Result<String> {
        Ok("ok".to_string())
    }
}

#[tokio::test]
async fn will_sends_report() {
    let (tx, mut rx) = broadcast::channel(8);
    let bus = TopicBus::new(8);
    let will = Will::with_debug(bus, Arc::new(Dummy), Some(tx.clone()));
    // no report when disabled
    will.observe(Impression::new(
        vec![Stimulus::new("go".to_string())],
        "",
        None::<String>,
    ))
    .await;
    let _ = will.tick().await;
    assert!(rx.try_recv().is_err());
    psyche::enable_debug("Will").await;
    will.observe(Impression::new(
        vec![Stimulus::new("go".to_string())],
        "",
        None::<String>,
    ))
    .await;
    let _ = will.tick().await;
    let report = rx.recv().await.unwrap();
    assert_eq!(report.name, "Will");
    assert!(report.prompt.contains("go") || report.prompt.contains("Go"));
    assert_eq!(report.output, "ok");
    psyche::disable_debug("Will").await;
}
