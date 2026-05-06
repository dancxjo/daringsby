use async_trait::async_trait;
use lingproc::LlmInstruction;
use psyche::model::localized_timestamp;
use psyche::topics::{Topic, TopicBus};
use psyche::traits::Doer;
use psyche::wits::MomentWit;
use psyche::{Impression, Stimulus, Wit};
use std::sync::Arc;
use tokio::time::{Duration, sleep};

#[derive(Clone)]
struct DummyDoer;

#[async_trait]
impl Doer for DummyDoer {
    async fn follow(&self, i: LlmInstruction) -> anyhow::Result<String> {
        Ok(format!("SUMMARY:{}", i.command))
    }
}

fn publish_instants(bus: &TopicBus, count: usize) {
    for i in 0..count {
        bus.publish(
            Topic::Instant,
            Impression::new(
                vec![Stimulus::new(format!("i{i}"))],
                format!("i{i}"),
                None::<String>,
            ),
        );
    }
}

#[tokio::test]
async fn publishes_summary_after_three_instants() {
    let bus = TopicBus::new(8);
    let wit = MomentWit::new(bus.clone(), Arc::new(DummyDoer));
    sleep(Duration::from_millis(20)).await;
    publish_instants(&bus, 3);
    sleep(Duration::from_millis(50)).await;
    let out = wit.tick().await;
    assert_eq!(out.len(), 1);
    assert!(out[0].summary.starts_with("SUMMARY:"));
}

#[tokio::test]
async fn does_nothing_with_fewer_instants() {
    let bus = TopicBus::new(8);
    let wit = MomentWit::new(bus.clone(), Arc::new(DummyDoer));
    sleep(Duration::from_millis(20)).await;
    publish_instants(&bus, 2);
    sleep(Duration::from_millis(50)).await;
    let out = wit.tick().await;
    assert!(out.is_empty());
}

#[tokio::test]
async fn debug_report_contains_prompt_and_summary() {
    let bus = TopicBus::new(8);
    let (tx, mut rx) = tokio::sync::broadcast::channel(8);
    psyche::enable_debug("MomentWit").await;
    let wit = MomentWit::with_debug(bus.clone(), Arc::new(DummyDoer), Some(tx));
    sleep(Duration::from_millis(20)).await;
    publish_instants(&bus, 3);
    sleep(Duration::from_millis(50)).await;
    let _ = wit.tick().await;
    let report = rx.recv().await.unwrap();
    assert_eq!(report.name, "MomentWit");
    assert!(report.prompt.contains("Summarize"));
    assert!(report.prompt.contains("one short first-person sentence"));
    assert!(report.prompt.contains("not the sensor stream"));
    assert!(
        report
            .prompt
            .contains("I cannot tell what is happening yet")
    );
    assert!(report.prompt.contains("do not enumerate ids"));
    assert!(report.output.contains("SUMMARY:"));
    psyche::disable_debug("MomentWit").await;
}

#[tokio::test]
async fn prompt_timestamps_recent_instants() {
    let bus = TopicBus::new(8);
    let wit = MomentWit::new(bus.clone(), Arc::new(DummyDoer));
    sleep(Duration::from_millis(20)).await;
    let timestamp = chrono::DateTime::parse_from_rfc3339("2026-05-05T12:34:56Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    for i in 0..3 {
        bus.publish(
            Topic::Instant,
            Impression {
                stimuli: vec![Stimulus {
                    what: format!("i{i}"),
                    timestamp,
                    source_sensation_ids: Vec::new(),
                }],
                source_sensation_ids: Vec::new(),
                summary: format!("i{i}"),
                emoji: None,
                timestamp,
            },
        );
    }
    sleep(Duration::from_millis(50)).await;
    let out = wit.tick().await;

    assert!(out[0].summary.contains(&localized_timestamp(timestamp)));
    assert!(out[0].summary.contains("i0"));
}
