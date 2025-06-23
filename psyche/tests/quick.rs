use async_trait::async_trait;
use lingproc::LlmInstruction;
use psyche::traits::Doer;
use psyche::wits::Quick;
use psyche::{Sensation, Topic, TopicBus, Wit};
use std::sync::Arc;
use tokio::time::{Duration, sleep};

#[derive(Clone)]
struct Dummy;

#[async_trait]
impl Doer for Dummy {
    async fn follow(&self, i: LlmInstruction) -> anyhow::Result<String> {
        Ok(format!("SUM:{}", i.command))
    }
}

#[tokio::test]
async fn summarizes_heard_text() {
    let bus = TopicBus::new(8);
    let quick = Quick::new(bus.clone(), Arc::new(Dummy));
    // allow subscriber spawn
    sleep(Duration::from_millis(20)).await;
    bus.publish(Topic::Sensation, Sensation::HeardUserVoice("hi".into()));
    sleep(Duration::from_millis(20)).await;
    let out = quick.tick().await;
    assert_eq!(out.len(), 1);
    assert!(out[0].summary.starts_with("SUM:"));
    assert_eq!(out[0].stimuli.len(), 1);
}
