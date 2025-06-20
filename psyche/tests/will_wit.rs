use async_trait::async_trait;
use psyche::ling::{ChatStream, Chatter, Doer, Instruction, Message};
use psyche::{
    Impression, Voice, Wit,
    wits::{Will, WillWit},
};
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_stream::once;

#[derive(Clone, Default)]
struct SpyLLM(Arc<tokio::sync::Mutex<Vec<String>>>);

#[derive(Clone, Default)]
struct RecMouth(Arc<tokio::sync::Mutex<Vec<String>>>);

#[async_trait]
impl psyche::Mouth for RecMouth {
    async fn speak(&self, t: &str) {
        self.0.lock().await.push(t.to_string());
    }
    async fn interrupt(&self) {}
    fn speaking(&self) -> bool {
        false
    }
}

#[async_trait]
impl Chatter for SpyLLM {
    async fn chat(&self, s: &str, _h: &[Message]) -> anyhow::Result<ChatStream> {
        self.0.lock().await.push(s.to_string());
        Ok(Box::pin(once(Ok("ok".into()))))
    }
    async fn update_prompt_context(&self, _c: &str) {}
}

#[async_trait]
impl Doer for SpyLLM {
    async fn follow(&self, _i: Instruction) -> anyhow::Result<String> {
        Ok("ok".into())
    }
}

#[tokio::test]
async fn emits_take_turn_every_third_tick() {
    let llm = Arc::new(SpyLLM::default());
    let mouth = Arc::new(RecMouth::default());
    let (tx, _rx) = broadcast::channel(8);
    let voice = Arc::new(Voice::new(llm.clone(), mouth, tx));
    voice.take_turn("init", &[]).await.unwrap();
    let will = Arc::new(Will::new(Box::new(SpyLLM::default())));
    let wit = WillWit::new(will, voice.clone());

    for _ in 0..2 {
        wit.observe(Impression::new("", None::<String>, "hi".into()))
            .await;
        let imps = wit.tick().await;
        assert!(imps.iter().all(|i| !i.raw_data.contains("<take_turn>")));
    }

    wit.observe(Impression::new("", None::<String>, "hey".into()))
        .await;
    let imps = wit.tick().await;
    assert!(imps.iter().any(|i| {
        i.raw_data
            .contains("<take_turn>share a brief update</take_turn>")
    }));
}
