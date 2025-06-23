use async_trait::async_trait;
use lingproc::{Chatter, Doer, Instruction, Message, TextStream, Vectorizer};
use psyche::traits::{Ear, Mouth};
use psyche::{DEFAULT_SYSTEM_PROMPT, Psyche};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Default)]
struct Dummy {
    speaking: std::sync::Arc<AtomicBool>,
}

#[async_trait]
impl Mouth for Dummy {
    async fn speak(&self, _t: &str) {
        self.speaking.store(true, Ordering::SeqCst);
    }
    async fn interrupt(&self) {
        self.speaking.store(false, Ordering::SeqCst);
    }
    fn speaking(&self) -> bool {
        self.speaking.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl Ear for Dummy {
    async fn hear_self_say(&self, _t: &str) {
        self.speaking.store(false, Ordering::SeqCst);
    }
    async fn hear_user_say(&self, _t: &str) {}
}

#[async_trait]
impl Doer for Dummy {
    async fn follow(&self, _: Instruction) -> anyhow::Result<String> {
        Ok("ok".into())
    }
}

#[async_trait]
impl Chatter for Dummy {
    async fn chat(&self, _: &str, _: &[Message]) -> anyhow::Result<TextStream> {
        Ok(Box::pin(tokio_stream::once(Ok("hi".to_string()))))
    }
}

#[async_trait]
impl Vectorizer for Dummy {
    async fn vectorize(&self, _: &str) -> anyhow::Result<Vec<f32>> {
        Ok(vec![0.0])
    }
}

#[test]
fn default_prompt_present() {
    let mouth = std::sync::Arc::new(Dummy::default());
    let ear = mouth.clone();
    let psyche = Psyche::new(
        Box::new(Dummy::default()),
        Box::new(Dummy::default()),
        Box::new(Dummy::default()),
        std::sync::Arc::new(psyche::NoopMemory),
        mouth,
        ear,
    );
    assert_eq!(psyche.system_prompt(), DEFAULT_SYSTEM_PROMPT);
}

#[test]
fn senses_are_described() {
    let mouth = std::sync::Arc::new(Dummy::default());
    let ear = mouth.clone();
    let mut psyche = Psyche::new(
        Box::new(Dummy::default()),
        Box::new(Dummy::default()),
        Box::new(Dummy::default()),
        std::sync::Arc::new(psyche::NoopMemory),
        mouth,
        ear,
    );
    psyche.add_sense("Heartbeat: Announces the time every 7 minutes.".into());
    let prompt = psyche.described_system_prompt();
    assert!(prompt.contains("Heartbeat"));
}
