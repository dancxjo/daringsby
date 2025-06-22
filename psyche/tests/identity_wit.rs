use async_trait::async_trait;
use psyche::ling::{Doer, Instruction};
use psyche::wit::{Moment, Wit};
use psyche::wits::{FondDuCoeur, IdentityWit};
use psyche::{Impression, Stimulus};
use tokio::sync::broadcast;

#[derive(Clone)]
struct Dummy;

#[async_trait]
impl Doer for Dummy {
    async fn follow(&self, i: Instruction) -> anyhow::Result<String> {
        Ok(format!("story:{}", i.command))
    }
}

#[tokio::test]
async fn summarizes_moments_into_story() {
    let (tx, mut rx) = broadcast::channel(8);
    psyche::enable_debug("Story").await;
    let summarizer = FondDuCoeur::with_debug(Box::new(Dummy), tx);
    let wit = IdentityWit::new(summarizer.clone());
    wit.observe(Impression::new(
        vec![Stimulus::new(Moment {
            summary: "Pete woke".into(),
        })],
        "m1",
        None::<String>,
    ))
    .await;
    let out = wit.tick().await;
    assert_eq!(out.len(), 1);
    let report = rx.recv().await.unwrap();
    assert_eq!(report.name, "Story");
    assert!(report.output.contains("story:"));
    // second tick should include previous story
    wit.observe(Impression::new(
        vec![Stimulus::new(Moment {
            summary: "Pete slept".into(),
        })],
        "m2",
        None::<String>,
    ))
    .await;
    let _ = wit.tick().await;
    assert!(summarizer.story().contains("story:"));
    psyche::disable_debug("Story").await;
}
