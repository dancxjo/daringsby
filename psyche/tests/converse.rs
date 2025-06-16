use async_trait::async_trait;
use psyche::ling::{Chatter, Doer, Message, Vectorizer};
use psyche::{Ear, Event, Mouth, Psyche, Sensation};
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
    async fn follow(&self, _: &str) -> anyhow::Result<String> {
        Ok("ok".into())
    }
}

#[async_trait]
impl Chatter for Dummy {
    async fn chat(&self, _: &str, _: &[Message]) -> anyhow::Result<psyche::ling::ChatStream> {
        Ok(Box::pin(tokio_stream::once(Ok("hello world".to_string()))))
    }
}

#[async_trait]
impl Vectorizer for Dummy {
    async fn vectorize(&self, _: &str) -> anyhow::Result<Vec<f32>> {
        Ok(vec![0.0])
    }
}

#[tokio::test]
async fn adds_message_after_voice_heard() {
    let mouth = std::sync::Arc::new(Dummy::default());
    let ear = mouth.clone();
    let mut psyche = Psyche::new(
        Box::new(Dummy::default()),
        Box::new(Dummy::default()),
        Box::new(Dummy::default()),
        mouth,
        ear,
    );
    psyche.set_turn_limit(1);
    psyche.set_system_prompt("sys");

    let mut events = psyche.subscribe();
    let input = psyche.input_sender();

    let handle = tokio::spawn(async move { psyche.run().await });

    let mut saw_chunk = false;
    while let Ok(evt) = events.recv().await {
        match evt {
            Event::StreamChunk(_) => saw_chunk = true,
            Event::IntentionToSay(msg) => {
                input.send(Sensation::HeardOwnVoice(msg)).unwrap();
                break;
            }
        }
    }

    let psyche = handle.await.unwrap();
    assert!(saw_chunk);
    let conv = psyche.conversation();
    let log_len = { conv.lock().await.all().len() };
    assert_eq!(log_len, 1);
}

#[tokio::test]
async fn interrupts_when_user_speaks() {
    let mouth = std::sync::Arc::new(Dummy::default());
    let ear = mouth.clone();
    let mut psyche = Psyche::new(
        Box::new(Dummy::default()),
        Box::new(Dummy::default()),
        Box::new(Dummy::default()),
        mouth.clone(),
        ear,
    );
    psyche.set_turn_limit(1);
    psyche.set_system_prompt("sys");

    let mut events = psyche.subscribe();
    let input = psyche.input_sender();

    let handle = tokio::spawn(async move { psyche.run().await });

    while let Ok(evt) = events.recv().await {
        if let Event::IntentionToSay(msg) = evt {
            assert!(mouth.speaking());
            input.send(Sensation::HeardUserVoice("hi".into())).unwrap();
            input.send(Sensation::HeardOwnVoice(msg)).unwrap();
            break;
        }
    }

    let _ = handle.await.unwrap();
    assert!(!mouth.speaking());
}

#[tokio::test]
async fn times_out_without_echo() {
    let mouth = std::sync::Arc::new(Dummy::default());
    let ear = mouth.clone();
    let mut psyche = Psyche::new(
        Box::new(Dummy::default()),
        Box::new(Dummy::default()),
        Box::new(Dummy::default()),
        mouth,
        ear,
    );
    psyche.set_turn_limit(1);
    psyche.set_system_prompt("sys");
    psyche.set_echo_timeout(std::time::Duration::from_millis(10));

    let handle = tokio::spawn(async move { psyche.run().await });
    let psyche = handle.await.unwrap();
    let conv = psyche.conversation();
    let log_len = { conv.lock().await.all().len() };
    assert_eq!(log_len, 1);
}
