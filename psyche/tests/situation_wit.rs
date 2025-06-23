use async_trait::async_trait;
use lingproc::LlmInstruction;
use psyche::topics::{Topic, TopicBus};
use psyche::traits::Doer;
use psyche::wits::SituationWit;
use psyche::{Impression, Stimulus, Wit};
use std::sync::Arc;
use tokio::time::{Duration, sleep};

#[derive(Clone)]
struct DummyDoer;

#[async_trait]
impl Doer for DummyDoer {
    async fn follow(&self, i: LlmInstruction) -> anyhow::Result<String> {
        Ok(format!("SUM:{}", i.command))
    }
}

fn publish_moments(bus: &TopicBus, count: usize) {
    for i in 0..count {
        bus.publish(
            Topic::Moment,
            Impression::new(
                vec![Stimulus::new(format!("m{i}"))],
                format!("m{i}"),
                None::<String>,
            ),
        );
    }
}

#[tokio::test]
async fn publishes_summary_after_three_moments() {
    let bus = TopicBus::new(8);
    let wit = SituationWit::new(bus.clone(), Arc::new(DummyDoer));
    sleep(Duration::from_millis(20)).await;
    publish_moments(&bus, 3);
    sleep(Duration::from_millis(50)).await;
    let out = wit.tick().await;
    assert_eq!(out.len(), 1);
    assert!(out[0].summary.contains("SUM:"));
}

#[tokio::test]
async fn does_nothing_with_fewer_moments() {
    let bus = TopicBus::new(8);
    let wit = SituationWit::new(bus.clone(), Arc::new(DummyDoer));
    sleep(Duration::from_millis(20)).await;
    publish_moments(&bus, 2);
    sleep(Duration::from_millis(50)).await;
    let out = wit.tick().await;
    assert!(out.is_empty());
}

#[tokio::test]
async fn debug_report_contains_prompt_and_summary() {
    let bus = TopicBus::new(8);
    let (tx, mut rx) = tokio::sync::broadcast::channel(8);
    psyche::enable_debug("SituationWit").await;
    let wit = SituationWit::with_debug(bus.clone(), Arc::new(DummyDoer), Some(tx));
    sleep(Duration::from_millis(20)).await;
    publish_moments(&bus, 3);
    sleep(Duration::from_millis(50)).await;
    let _ = wit.tick().await;
    let report = rx.recv().await.unwrap();
    assert_eq!(report.name, "SituationWit");
    assert!(report.prompt.contains("recent moments"));
    assert!(report.output.contains("SUM:"));
    psyche::disable_debug("SituationWit").await;
}
