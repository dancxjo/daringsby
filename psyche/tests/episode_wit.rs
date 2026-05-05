use async_trait::async_trait;
use lingproc::LlmInstruction;
use psyche::model::localized_timestamp;
use psyche::topics::{Topic, TopicBus};
use psyche::traits::Doer;
use psyche::wits::EpisodeWit;
use psyche::{HostInstruction, Impression, Stimulus, Wit};
use std::sync::Arc;
use tokio::time::{Duration, sleep};

#[derive(Clone)]
struct DummyDoer;

#[async_trait]
impl Doer for DummyDoer {
    async fn follow(&self, i: LlmInstruction) -> anyhow::Result<String> {
        Ok(format!("EPISUM:{}", i.command))
    }
}

fn publish_situations(bus: &TopicBus, count: usize) {
    for i in 0..count {
        bus.publish(
            Topic::Situation,
            Impression::new(
                vec![Stimulus::new(format!("s{i}"))],
                format!("s{i}"),
                None::<String>,
            ),
        );
    }
}

#[tokio::test]
async fn emits_summary_on_break() {
    let bus = TopicBus::new(8);
    let wit = EpisodeWit::new(bus.clone(), Arc::new(DummyDoer));
    sleep(Duration::from_millis(20)).await;
    publish_situations(&bus, 3);
    sleep(Duration::from_millis(20)).await;
    bus.publish(Topic::Instruction, HostInstruction::BreakEpisode);
    sleep(Duration::from_millis(20)).await;
    let out = wit.tick().await;
    assert_eq!(out.len(), 1);
    assert!(out[0].summary.contains("EPISUM:"));
}

#[tokio::test]
async fn no_emit_without_break_or_enough_items() {
    let bus = TopicBus::new(8);
    let wit = EpisodeWit::new(bus.clone(), Arc::new(DummyDoer));
    sleep(Duration::from_millis(20)).await;
    publish_situations(&bus, 2);
    sleep(Duration::from_millis(20)).await;
    let out = wit.tick().await;
    assert!(out.is_empty());
}

#[tokio::test]
async fn empty_buffer_emits_nothing() {
    let bus = TopicBus::new(8);
    let wit = EpisodeWit::new(bus.clone(), Arc::new(DummyDoer));
    sleep(Duration::from_millis(20)).await;
    let out = wit.tick().await;
    assert!(out.is_empty());
}

#[tokio::test]
async fn prompt_timestamps_recent_situations() {
    let bus = TopicBus::new(8);
    let wit = EpisodeWit::new(bus.clone(), Arc::new(DummyDoer));
    sleep(Duration::from_millis(20)).await;
    let timestamp = chrono::DateTime::parse_from_rfc3339("2026-05-05T12:34:56Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    for i in 0..3 {
        bus.publish(
            Topic::Situation,
            Impression {
                stimuli: vec![Stimulus {
                    what: format!("s{i}"),
                    timestamp,
                }],
                summary: format!("s{i}"),
                emoji: None,
                timestamp,
            },
        );
    }
    sleep(Duration::from_millis(50)).await;
    let out = wit.tick().await;

    assert!(out[0].summary.contains(&localized_timestamp(timestamp)));
    assert!(out[0].summary.contains("s0"));
}
