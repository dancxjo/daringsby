use async_trait::async_trait;
use lingproc::{Doer, Instruction};
use psyche::{Impression, Stimulus, Summarizer, WillSummarizer, WitReport};
use tokio::sync::broadcast;

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
    let will = WillSummarizer::with_debug(Box::new(Dummy), tx.clone());
    // no report when disabled
    let _ = will
        .digest(&[Impression::new(
            vec![Stimulus::new("go".to_string())],
            "",
            None::<String>,
        )])
        .await
        .unwrap();
    assert!(rx.try_recv().is_err());
    psyche::enable_debug("Will").await;
    let _ = will
        .digest(&[Impression::new(
            vec![Stimulus::new("go".to_string())],
            "",
            None::<String>,
        )])
        .await
        .unwrap();
    let report = rx.recv().await.unwrap();
    assert_eq!(report.name, "Will");
    assert!(report.prompt.contains("go") || report.prompt.contains("Go"));
    assert_eq!(report.output, "ok");
    psyche::disable_debug("Will").await;
}
