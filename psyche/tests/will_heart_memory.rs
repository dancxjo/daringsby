//! Integration tests exercising Will, Heart and Memory together.

use async_trait::async_trait;
use chrono::Utc;
use psyche::ling::{Chatter, Doer, Instruction, Message};
use psyche::{Countenance, Ear, Event, Heart, Impression, Memory, Mouth, Summarizer, Will};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::{Mutex, broadcast};
use uuid::Uuid;

struct DummyMouth;

#[async_trait]
impl Mouth for DummyMouth {
    async fn speak(&self, text: &str) {
        println!("Speaking: {}", text);
    }
    async fn interrupt(&self) {}
    fn speaking(&self) -> bool {
        false
    }
}

struct DummyEar;

#[async_trait]
impl Ear for DummyEar {
    async fn hear_self_say(&self, _text: &str) {}
    async fn hear_user_say(&self, _text: &str) {}
}

struct DummyCountenance;

#[async_trait]
impl Countenance for DummyCountenance {
    fn express(&self, emoji: &str) {
        println!("Emotion changed to: {}", emoji);
    }
}

struct DummyVoice;

#[async_trait]
impl Chatter for DummyVoice {
    async fn chat(
        &self,
        _prompt: &str,
        _history: &[Message],
    ) -> anyhow::Result<psyche::ling::ChatStream> {
        Ok(Box::pin(tokio_stream::once(Ok(
            "This is a test response.".to_string()
        ))))
    }
}

struct DummyDoer;

#[async_trait]
impl Doer for DummyDoer {
    async fn follow(&self, _instruction: Instruction) -> anyhow::Result<String> {
        Ok("Executed".to_string())
    }
}

#[tokio::test]
async fn will_can_invoke_voice() {
    let doer = Box::new(DummyDoer);
    let will = Will::new(doer);
    let imp = will
        .digest(&[Impression {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            headline: "".into(),
            details: None,
            raw_data: "Now is the time.".to_string(),
        }])
        .await
        .unwrap();
    assert_eq!(imp.raw_data, "Executed");
}

#[tokio::test]
async fn heart_sets_emotion() {
    let doer = Box::new(DummyDoer);
    let heart = Heart::new(doer);
    let imp = heart
        .digest(&[Impression {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            headline: "".into(),
            details: None,
            raw_data: "You are feeling happy.".to_string(),
        }])
        .await
        .unwrap();
    assert_eq!(imp.raw_data, "Executed");
}

#[tokio::test]
async fn memory_store_invoked() {
    #[derive(Default)]
    struct MockMemory(std::sync::Mutex<usize>);

    #[async_trait]
    impl Memory for MockMemory {
        async fn store(&self, _: &Impression<Value>) -> anyhow::Result<()> {
            let mut guard = self.0.lock().unwrap();
            *guard += 1;
            Ok(())
        }
    }

    let memory = MockMemory::default();
    <dyn Memory>::store_serializable(&memory, &Impression::new("hey", None::<String>, ()))
        .await
        .unwrap();
    assert_eq!(*memory.0.lock().unwrap(), 1);
}
