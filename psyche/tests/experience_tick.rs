use async_trait::async_trait;
use lingproc::{Chatter, Doer, Instruction, Message, TextStream, Vectorizer};
use psyche::traits::{Ear, Mouth};
use psyche::{Impression, Psyche, wit::Wit};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;
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
    async fn follow(&self, _i: Instruction) -> anyhow::Result<String> {
        Ok("ok".into())
    }
}

#[async_trait]
impl Vectorizer for Dummy {
    async fn vectorize(&self, _t: &str) -> anyhow::Result<Vec<f32>> {
        Ok(vec![0.0])
    }
}

struct CountingWit(AtomicUsize);

#[async_trait]
impl Wit<(), ()> for CountingWit {
    async fn observe(&self, _: ()) {}
    async fn tick(&self) -> Vec<Impression<()>> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Vec::new()
    }
}

#[tokio::test]
async fn experience_tick_configurable() {
    let mouth = Arc::new(Dummy::default());
    let ear = mouth.clone();
    let mut psyche = Psyche::new(
        Box::new(Dummy::default()),
        Box::new(Dummy::default()),
        Box::new(Dummy::default()),
        Arc::new(psyche::NoopMemory),
        mouth,
        ear,
    );
    psyche.set_turn_limit(3);
    psyche.set_speak_when_spoken_to(true);
    psyche.set_experience_tick(Duration::from_millis(50));
    let wit = Arc::new(CountingWit(AtomicUsize::new(0)));
    psyche.register_typed_wit(wit.clone());
    let handle = tokio::spawn(async move { psyche.run().await });
    tokio::time::sleep(Duration::from_millis(120)).await;
    handle.abort();
    let _ = handle.await;
    assert!(wit.0.load(Ordering::SeqCst) >= 2);
}
