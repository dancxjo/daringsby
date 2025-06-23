use async_trait::async_trait;
use futures::{StreamExt, pin_mut};
use lingproc::{Chatter, Doer, Instruction, Message, TextStream, Vectorizer};
use psyche::{Ear, Mouth, Psyche, Topic};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Default)]
struct Dummy {
    speaking: Arc<AtomicBool>,
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

#[tokio::test]
async fn feel_forwards_to_topic_bus() {
    let mouth = Arc::new(Dummy::default());
    let ear = mouth.clone();
    let psyche = Psyche::new(
        Box::new(Dummy::default()),
        Box::new(Dummy::default()),
        Box::new(Dummy::default()),
        Arc::new(psyche::NoopMemory),
        mouth,
        ear,
    );
    let bus = psyche.topic_bus();
    let mut sub = bus.subscribe(Topic::Sensation);
    pin_mut!(sub);
    psyche.feel("hello".to_string());
    let payload = sub.next().await.unwrap();
    let text = payload.downcast::<String>().unwrap();
    assert_eq!(&*text, "hello");
}
