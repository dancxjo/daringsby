use async_trait::async_trait;
use psyche::ling::{Chatter, Doer, Instruction, Message, TextStream, Vectorizer};
use psyche::{Ear, Impression, Mouth, Psyche, Sensation, Stimulus, wit::Wit};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;
use tokio::sync::{Mutex as TokioMutex, broadcast};
use tokio_stream::once;

#[derive(Clone, Default)]
struct RecLLM(Arc<TokioMutex<Vec<String>>>);

#[async_trait]
impl Chatter for RecLLM {
    async fn chat(&self, s: &str, _h: &[Message]) -> anyhow::Result<TextStream> {
        self.0.lock().await.push(s.to_string());
        Ok(Box::pin(once(Ok("ok".into()))))
    }
    async fn update_prompt_context(&self, _c: &str) {}
}

#[async_trait]
impl Doer for RecLLM {
    async fn follow(&self, _i: Instruction) -> anyhow::Result<String> {
        Ok("ok".into())
    }
}

#[async_trait]
impl Vectorizer for RecLLM {
    async fn vectorize(&self, _t: &str) -> anyhow::Result<Vec<f32>> {
        Ok(vec![0.0])
    }
}

#[derive(Clone, Default)]
struct RecMouth(Arc<TokioMutex<Vec<String>>>);

#[async_trait]
impl Mouth for RecMouth {
    async fn speak(&self, t: &str) {
        self.0.lock().await.push(t.to_string());
    }
    async fn interrupt(&self) {}
    fn speaking(&self) -> bool {
        false
    }
}

#[derive(Clone, Default)]
struct DummyEar;

#[async_trait]
impl Ear for DummyEar {
    async fn hear_self_say(&self, _t: &str) {}
    async fn hear_user_say(&self, _t: &str) {}
}

struct TakeTurnWit(AtomicBool);

#[async_trait]
impl Wit<(), String> for TakeTurnWit {
    async fn observe(&self, _: ()) {}
    async fn tick(&self) -> Vec<Impression<String>> {
        if self.0.swap(true, Ordering::SeqCst) {
            Vec::new()
        } else {
            vec![Impression::new(
                vec![Stimulus::new("<take_turn>hi</take_turn>".to_string())],
                "turn",
                None::<String>,
            )]
        }
    }
}

#[tokio::test]
async fn no_speech_without_command() {
    let mouth_rec = Arc::new(RecMouth::default());
    let (_tx, _rx) = broadcast::channel::<psyche::Event>(8);
    let voice = Box::new(RecLLM::default()) as Box<dyn Chatter>;
    let mouth = mouth_rec.clone() as Arc<dyn Mouth>;
    let ear = Arc::new(DummyEar) as Arc<dyn Ear>;
    let mut psyche = Psyche::new(
        Box::new(RecLLM::default()),
        voice,
        Box::new(RecLLM::default()),
        Arc::new(psyche::NoopMemory),
        mouth,
        ear,
    );
    psyche.set_turn_limit(1);
    psyche.set_fallback_turn_enabled(false);
    psyche.set_fallback_turn_enabled(false);
    let input = psyche.input_sender();
    let handle = tokio::spawn(async move { psyche.run().await });
    input
        .send(Sensation::HeardUserVoice("hi".into()))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    handle.abort();
    let _ = handle.await;
    assert!(mouth_rec.0.lock().await.is_empty());
}

#[tokio::test]
async fn speaks_when_commanded() {
    let mouth_rec = Arc::new(RecMouth::default());
    let (_tx, _rx) = broadcast::channel::<psyche::Event>(8);
    let voice = Box::new(RecLLM::default()) as Box<dyn Chatter>;
    let mouth = mouth_rec.clone() as Arc<dyn Mouth>;
    let ear = Arc::new(DummyEar) as Arc<dyn Ear>;
    let mut psyche = Psyche::new(
        Box::new(RecLLM::default()),
        voice,
        Box::new(RecLLM::default()),
        Arc::new(psyche::NoopMemory),
        mouth,
        ear,
    );
    psyche.set_turn_limit(1);
    psyche.register_typed_wit(Arc::new(TakeTurnWit(AtomicBool::new(false))));
    let handle = tokio::spawn(async move { psyche.run().await });
    tokio::time::sleep(Duration::from_millis(50)).await;
    handle.abort();
    let _ = handle.await;
    assert!(!mouth_rec.0.lock().await.is_empty());
}

#[tokio::test]
async fn speaks_with_fallback_when_no_wit() {
    let mouth_rec = Arc::new(RecMouth::default());
    let (_tx, _rx) = broadcast::channel::<psyche::Event>(8);
    let voice = Box::new(RecLLM::default()) as Box<dyn Chatter>;
    let mouth = mouth_rec.clone() as Arc<dyn Mouth>;
    let ear = Arc::new(DummyEar) as Arc<dyn Ear>;
    let mut psyche = Psyche::new(
        Box::new(RecLLM::default()),
        voice,
        Box::new(RecLLM::default()),
        Arc::new(psyche::NoopMemory),
        mouth,
        ear,
    );
    psyche.set_turn_limit(1);
    // fallback enabled by default
    let input = psyche.input_sender();
    let handle = tokio::spawn(async move { psyche.run().await });
    input
        .send(Sensation::HeardUserVoice("hi".into()))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    handle.abort();
    let _ = handle.await;
    assert!(!mouth_rec.0.lock().await.is_empty());
}
