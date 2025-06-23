use async_trait::async_trait;
use lingproc::{Chatter, Doer, LlmInstruction, Message, TextStream, Vectorizer};
use psyche::wits::memory::Memory;
use psyche::{Ear, Mouth, Psyche};
use serde_json::Value;
use std::sync::Arc;
use tokio_stream::once;

#[derive(Clone, Default)]
struct Dummy;

#[async_trait]
impl Mouth for Dummy {
    async fn speak(&self, _t: &str) {}
    async fn interrupt(&self) {}
    fn speaking(&self) -> bool {
        false
    }
}

#[async_trait]
impl Ear for Dummy {
    async fn hear_self_say(&self, _t: &str) {}
    async fn hear_user_say(&self, _t: &str) {}
}

#[async_trait]
impl Chatter for Dummy {
    async fn chat(&self, _s: &str, _h: &[Message]) -> anyhow::Result<lingproc::TextStream> {
        Ok(Box::pin(once(Ok("ok".into()))))
    }
    async fn update_prompt_context(&self, _c: &str) {}
}

#[async_trait]
impl Doer for Dummy {
    async fn follow(&self, _i: LlmInstruction) -> anyhow::Result<String> {
        Ok("ok".into())
    }
}

#[async_trait]
impl Vectorizer for Dummy {
    async fn vectorize(&self, _t: &str) -> anyhow::Result<Vec<f32>> {
        Ok(vec![0.0])
    }
}

#[async_trait]
impl Memory for Dummy {
    async fn store(&self, _i: &psyche::Impression<serde_json::Value>) -> anyhow::Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn custom_channel_capacity() {
    let capacity = 32;
    let psyche = Psyche::with_channel_capacity(
        Box::new(Dummy),
        Box::new(Dummy),
        Box::new(Dummy),
        Arc::new(Dummy),
        Arc::new(Dummy),
        Arc::new(Dummy),
        capacity,
    );
    // Custom channel capacity constructor should behave like default
    let _ = psyche
        .event_sender()
        .send(psyche::Event::EmotionChanged("🙂".into()));
    let _ = psyche.wit_sender();
}
