use async_trait::async_trait;
use psyche::ling::{ChatContext, Chatter, Doer, Message, Vectorizer};
use psyche::{Countenance, NoopCountenance, Psyche};
use psyche::{Ear, Mouth};
use std::sync::{Arc, Mutex};

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
impl Doer for Dummy {
    async fn follow(&self, _: &str) -> anyhow::Result<String> {
        Ok("ok".into())
    }
}

#[async_trait]
impl Chatter for Dummy {
    async fn chat(&self, _: ChatContext<'_>) -> anyhow::Result<psyche::ling::ChatStream> {
        Ok(Box::pin(tokio_stream::empty()))
    }
}

#[async_trait]
impl Vectorizer for Dummy {
    async fn vectorize(&self, _: &str) -> anyhow::Result<Vec<f32>> {
        Ok(vec![0.0])
    }
}

#[derive(Clone, Default)]
struct RecordingFace(Arc<Mutex<Option<String>>>);

impl Countenance for RecordingFace {
    fn express(&self, emoji: &str) {
        *self.0.lock().unwrap() = Some(emoji.to_string());
    }
}

#[test]
fn set_emotion_calls_countenance() {
    let mouth = Arc::new(Dummy);
    let ear = Arc::new(Dummy);
    let mut psyche = Psyche::new(
        Box::new(Dummy),
        Box::new(Dummy),
        Box::new(Dummy),
        mouth,
        ear,
    );
    let face = Arc::new(RecordingFace::default());
    psyche.set_countenance(face.clone());
    psyche.set_emotion("😠");
    assert_eq!(face.0.lock().unwrap().as_deref(), Some("😠"));
}
