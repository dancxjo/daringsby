use async_trait::async_trait;
use psyche::Impression;
use psyche::ling::{Doer, Instruction};
use psyche::wit::{Moment, Wit};
use psyche::wits::{FondDuCoeur, FondDuCoeurWit};
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
    let summarizer = FondDuCoeur::with_debug(Box::new(Dummy), tx);
    let wit = FondDuCoeurWit::new(summarizer.clone());
    wit.observe(Impression::new(
        "m1",
        None::<String>,
        Moment {
            summary: "Pete woke".into(),
        },
    ))
    .await;
    let out = wit.tick().await;
    assert_eq!(out.len(), 1);
    let report = rx.recv().await.unwrap();
    assert_eq!(report.name, "Story");
    assert!(report.output.contains("story:"));
    // second tick should include previous story
    wit.observe(Impression::new(
        "m2",
        None::<String>,
        Moment {
            summary: "Pete slept".into(),
        },
    ))
    .await;
    let _ = wit.tick().await;
    assert!(summarizer.story().contains("story:"));
}
