use async_trait::async_trait;
use futures::StreamExt;
use psyche::ling::{Doer, Instruction as LlmInstruction};
use psyche::topics::{Topic, TopicBus};
use psyche::{Impression, Stimulus, Wit};
use psyche::{Instruction, wits::WillWit};
use std::sync::Arc;

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
