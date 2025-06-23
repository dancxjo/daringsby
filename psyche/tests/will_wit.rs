use async_trait::async_trait;
use futures::StreamExt;
use lingproc::LlmInstruction;
use psyche::topics::{Topic, TopicBus};
use psyche::traits::Doer;
use psyche::{HostInstruction, wits::Will};
use psyche::{Impression, Stimulus, Wit};
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

#[tokio::test]
async fn publishes_parsed_instructions() {
    let bus = TopicBus::new(8);
    let wit = Will::new(bus.clone(), Arc::new(DummyDoer("<say>Hello</say>")));
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    wit.observe(Impression::new(
        vec![Stimulus::new("hi".to_string())],
        "hi",
        None::<String>,
    ))
    .await;
    let sub = bus.subscribe(Topic::Instruction);
    tokio::pin!(sub);
    let out = wit.tick().await;
    assert!(matches!(
        out[0].stimuli[0].what.instructions[0],
        HostInstruction::Say { .. }
    ));
    let payload = sub.next().await.unwrap();
    let ins = payload.downcast::<HostInstruction>().unwrap();
    assert!(matches!(&*ins, HostInstruction::Say { text, .. } if text == "Hello"));
}

#[tokio::test]
async fn handles_invalid_xml_gracefully() {
    let bus = TopicBus::new(8);
    let wit = Will::new(bus.clone(), Arc::new(DummyDoer("<<bad")));
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    wit.observe(Impression::new(
        vec![Stimulus::new("hi".to_string())],
        "hi",
        None::<String>,
    ))
    .await;
    let imps = wit.tick().await;
    assert!(imps.is_empty());
}

#[tokio::test]
async fn mixed_instructions() {
    let bus = TopicBus::new(8);
    let wit = Will::new(
        bus.clone(),
        Arc::new(DummyDoer("<say>hi</say><move to=\"dock\" />")),
    );
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    wit.observe(Impression::new(
        vec![Stimulus::new("hi".to_string())],
        "hi",
        None::<String>,
    ))
    .await;
    let sub = bus.subscribe(Topic::Instruction);
    tokio::pin!(sub);
    let out = wit.tick().await;
    assert_eq!(out[0].stimuli[0].what.instructions.len(), 2);
    let first = sub
        .next()
        .await
        .unwrap()
        .downcast::<HostInstruction>()
        .unwrap();
    assert!(matches!(*first, HostInstruction::Say { .. }));
    let second = sub
        .next()
        .await
        .unwrap()
        .downcast::<HostInstruction>()
        .unwrap();
    assert!(matches!(*second, HostInstruction::Move { .. }));
}

#[tokio::test]
async fn empty_response_yields_nothing() {
    let bus = TopicBus::new(8);
    let wit = Will::new(bus.clone(), Arc::new(DummyDoer("")));
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    wit.observe(Impression::new(
        vec![Stimulus::new("hi".to_string())],
        "hi",
        None::<String>,
    ))
    .await;
    let out = wit.tick().await;
    assert!(out.is_empty());
    let sub = bus.subscribe(Topic::Instruction);
    tokio::pin!(sub);
    time::sleep(Duration::from_millis(20)).await;
    assert!(
        time::timeout(Duration::from_millis(10), sub.next())
            .await
            .is_err()
    );
}
