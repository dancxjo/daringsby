use async_trait::async_trait;
use psyche::ling::{Chatter, Doer, Instruction, Message, Vectorizer};
use psyche::{Ear, Mouth, Psyche};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio_stream::once;

#[derive(Clone, Default)]
struct Dummy {
    running: Arc<AtomicBool>,
}

#[async_trait]
impl Mouth for Dummy {
    async fn speak(&self, _text: &str) {
        self.running.store(true, Ordering::SeqCst);
    }
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
    async fn chat(&self, _s: &str, _h: &[Message]) -> anyhow::Result<psyche::ling::ChatStream> {
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

#[tokio::test]
async fn psyche_loops_stay_alive_when_idle() {
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
    let handle = tokio::spawn(async move { psyche.run().await });
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    assert!(!handle.is_finished());
    handle.abort();
    let _ = handle.await;
}
