use async_trait::async_trait;
use futures::StreamExt;
use psyche::ling::{Doer, Instruction as LlmInstruction};
use psyche::topics::{Topic, TopicBus};
use psyche::{Impression, Stimulus, Wit};
use psyche::{Instruction, wits::WillWit};
use std::sync::Arc;
use tokio::time::{self, Duration};

#[derive(Clone)]
struct DummyDoer(&'static str);

#[async_trait]
impl Doer for DummyDoer {
    async fn follow(&self, _i: LlmInstruction) -> anyhow::Result<String> {
        Ok(self.0.to_string())
    }
}

fn publish_sample(bus: &TopicBus) {
    bus.publish(
        Topic::Moment,
        Impression::new(vec![Stimulus::new("hi".to_string())], "hi", None::<String>),
    );
}

#[tokio::test]
async fn publishes_parsed_instructions() {
    let bus = TopicBus::new(8);
    let wit = WillWit::new(bus.clone(), Arc::new(DummyDoer("<say>Hello</say>")));
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    publish_sample(&bus);
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let mut sub = bus.subscribe(Topic::Instruction);
    tokio::pin!(sub);
    let out = wit.tick().await;
    assert!(matches!(out[0].stimuli[0].what, Instruction::Say { .. }));
    let payload = sub.next().await.unwrap();
    let ins = payload.downcast::<Instruction>().unwrap();
    assert!(matches!(&*ins, Instruction::Say { text, .. } if text == "Hello"));
}

#[tokio::test]
async fn handles_invalid_xml_gracefully() {
    let bus = TopicBus::new(8);
    let wit = WillWit::new(bus.clone(), Arc::new(DummyDoer("<<bad")));
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    publish_sample(&bus);
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let imps = wit.tick().await;
    assert!(imps.is_empty());
}

#[tokio::test]
async fn mixed_instructions() {
    let bus = TopicBus::new(8);
    let wit = WillWit::new(
        bus.clone(),
        Arc::new(DummyDoer("<say>hi</say><move to=\"dock\" />")),
    );
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    publish_sample(&bus);
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let mut sub = bus.subscribe(Topic::Instruction);
    tokio::pin!(sub);
    let out = wit.tick().await;
    assert_eq!(out.len(), 2);
    let first = sub.next().await.unwrap().downcast::<Instruction>().unwrap();
    assert!(matches!(*first, Instruction::Say { .. }));
    let second = sub.next().await.unwrap().downcast::<Instruction>().unwrap();
    assert!(matches!(*second, Instruction::Move { .. }));
}

#[tokio::test]
async fn empty_response_yields_nothing() {
    let bus = TopicBus::new(8);
    let wit = WillWit::new(bus.clone(), Arc::new(DummyDoer("")));
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    publish_sample(&bus);
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let out = wit.tick().await;
    assert!(out.is_empty());
    let mut sub = bus.subscribe(Topic::Instruction);
    tokio::pin!(sub);
    time::sleep(Duration::from_millis(20)).await;
    assert!(
        time::timeout(Duration::from_millis(10), sub.next())
            .await
            .is_err()
    );
}
