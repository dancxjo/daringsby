use async_trait::async_trait;
use psyche::ling::{Doer, Instruction};
use psyche::wits::HeartWit;
use psyche::{Impression, Motor, Wit};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct DummyLLM;

#[async_trait]
impl Doer for DummyLLM {
    async fn follow(&self, _i: Instruction) -> anyhow::Result<String> {
        Ok("😊".to_string())
    }
}

#[derive(Default)]
struct RecordingMotor(Arc<Mutex<Vec<String>>>);

#[async_trait]
impl Motor for RecordingMotor {
    async fn say(&self, _text: &str) {}
    async fn set_emotion(&self, emoji: &str) {
        self.0.lock().unwrap().push(emoji.to_string());
    }
    async fn take_photo(&self) {}
    async fn focus_on(&self, _name: &str) {}
}

#[tokio::test]
async fn updates_emotion_on_tick() {
    let motor = Arc::new(RecordingMotor::default());
    let wit = HeartWit::new(Box::new(DummyLLM), motor.clone());
    wit.observe(Impression::new("", None::<String>, "test".to_string()))
        .await;
    let out = wit.tick().await;
    assert_eq!(out.len(), 1);
    let emos = motor.0.lock().unwrap().clone();
    assert_eq!(emos, vec!["😊".to_string()]);
}
